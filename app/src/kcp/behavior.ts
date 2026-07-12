// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * BEHAVIOR group (0x7x) wire helpers. The SOCD, key-override, tap-dance and
 * combo layouts mirror `kcp.rs`'s `behavior_dispatch` and its `pack_*` helpers
 * byte-for-byte; keycodes cross the wire as little-endian `u16`s, matching the
 * KEYMAP group. The table capacities are read at runtime — SOCD / override from
 * BEHAVIOR_INFO, tap-dance / combo from TIMED_INFO — rather than hard-coded.
 *
 * Ops (low nibble of CMD):
 *  - SOCD_SET (0x70): request `[index, a_lo, a_hi, b_lo, b_hi, mode]`.
 *  - SOCD_CLEAR (0x71): request `[index]` (0xFF clears the whole table).
 *  - SOCD_GET (0x72): request `[index]`; reply `[present, a_lo, a_hi, b_lo, b_hi, mode]`.
 *  - OVERRIDE_SET (0x73): request `[index, trig_lo, trig_hi, trig_mods, repl_lo,
 *    repl_hi, repl_mods, layer_lo, layer_hi, enabled]`.
 *  - OVERRIDE_CLEAR (0x74): request `[index]` (0xFF clears the whole table).
 *  - OVERRIDE_GET (0x75): request `[index]`; reply `[present, …]` (same 9 fields).
 *  - BEHAVIOR_INFO (0x76): no payload; reply `[MAX_SOCD, MAX_OVERRIDES]`.
 *  - TAPDANCE_SET (0x77): request `[index, tap, hold, double, term]` (each u16 LE).
 *  - TAPDANCE_GET (0x78): request `[index]`; reply `[present, tap, hold, double, term]`.
 *  - TAPDANCE_CLEAR (0x79): request `[index]` (0xFF clears the whole table).
 *  - COMBO_SET (0x7A): request `[index, len, k0, k1, k2, k3, action, term]`.
 *  - COMBO_GET (0x7B): request `[index]`; reply `[present, len, k0..k3, action, term]`.
 *  - COMBO_CLEAR (0x7C): request `[index]` (0xFF clears the whole table).
 *  - TIMED_INFO (0x7D): no payload; reply `[MAX_TAP_DANCE, MAX_COMBO,
 *    MAX_COMBO_KEYS, MAX_MACRO, MAX_MACRO_STEPS, MAX_LEADER, MAX_LEADER_SEQ]`.
 *  - LEADER_SET (0x7E): request `[index, len, s0..s4, action]` (len 0 clears the slot).
 *  - LEADER_GET (0x7F): request `[index]`; reply `[len, s0..s4, action]`.
 */
import { readU16LE } from './bytes';

/**
 * SOCD resolution modes (`behavior::SocdMode`; the SOCD_SET `mode` byte):
 * LastWins = 0, Neutral = 1, FirstWins = 2. The firmware's `SocdMode::from_u8`
 * rejects any other value with BadArg.
 */
export const SocdMode = {
  LastWins: 0,
  Neutral: 1,
  FirstWins: 2,
} as const;

export type SocdModeValue = (typeof SocdMode)[keyof typeof SocdMode];

/** SOCD modes in UI order, with labels and a one-line description. */
export const SOCD_MODES: readonly { mode: SocdModeValue; label: string; hint: string }[] = [
  { mode: SocdMode.LastWins, label: 'Last wins', hint: 'Most recent press wins (null-bind)' },
  { mode: SocdMode.Neutral, label: 'Neutral', hint: 'Suppress both while both held' },
  { mode: SocdMode.FirstWins, label: 'First wins', hint: 'First press wins; later is dropped' },
];

/**
 * Sentinel index for SOCD_CLEAR / OVERRIDE_CLEAR that clears the whole table
 * instead of one slot (`behavior::CLEAR_ALL`). It is outside every valid slot.
 */
export const CLEAR_ALL = 0xff;

/**
 * HID modifier-byte bits (usages `0xE0..0xE7` as bits `0..7`), used by the key
 * override editor for the trigger / replacement modifier bytes.
 */
export const MODIFIER_BITS: readonly { bit: number; label: string; name: string }[] = [
  { bit: 0, label: 'LCtrl', name: 'Left Control' },
  { bit: 1, label: 'LShift', name: 'Left Shift' },
  { bit: 2, label: 'LAlt', name: 'Left Alt' },
  { bit: 3, label: 'LCmd', name: 'Left GUI' },
  { bit: 4, label: 'RCtrl', name: 'Right Control' },
  { bit: 5, label: 'RShift', name: 'Right Shift' },
  { bit: 6, label: 'RAlt', name: 'Right Alt' },
  { bit: 7, label: 'RCmd', name: 'Right GUI' },
];

/** Format a modifier byte as e.g. `LCtrl+LShift`, or `none` when no bit is set. */
export function formatModifiers(mods: number): string {
  const parts = MODIFIER_BITS.filter((m) => (mods & (1 << m.bit)) !== 0).map((m) => m.label);
  return parts.length ? parts.join('+') : 'none';
}

// === SOCD ==================================================================

/** A configured SOCD pair (mirror of `behavior::SocdPair`). Keycodes are raw u16. */
export interface SocdPair {
  /** First key of the pair (raw keycode). */
  a: number;
  /** Second, opposing key (raw keycode). */
  b: number;
  /** Resolution mode ({@link SocdMode}). */
  mode: number;
}

/** Build a `[index]` request payload (SOCD_GET / SOCD_CLEAR, OVERRIDE_GET / CLEAR). */
export function encodeIndexArg(index: number): number[] {
  return [index & 0xff];
}

/** Build the SOCD_SET request payload `[index, a_lo, a_hi, b_lo, b_hi, mode]`. */
export function encodeSocdSetArgs(index: number, pair: SocdPair): number[] {
  return [
    index & 0xff,
    pair.a & 0xff,
    (pair.a >> 8) & 0xff,
    pair.b & 0xff,
    (pair.b >> 8) & 0xff,
    pair.mode & 0xff,
  ];
}

/**
 * Parse a SOCD_GET reply `[present, a_lo, a_hi, b_lo, b_hi, mode]`. Returns
 * `null` for an empty slot (`present == 0`).
 */
export function parseSocdPair(payload: Uint8Array): SocdPair | null {
  if (payload[0] === 0) {
    return null;
  }
  return {
    a: readU16LE(payload, 1),
    b: readU16LE(payload, 3),
    mode: payload[5],
  };
}

// === Key overrides =========================================================

/** A configured key override (mirror of `behavior::KeyOverride`). */
export interface KeyOverride {
  /** Key that must be held for the override to fire (raw keycode). */
  trigger: number;
  /** Modifier byte that must be held *exactly* (HID modifier bits). */
  triggerMods: number;
  /** Key substituted for the trigger when the override fires (raw keycode). */
  replacement: number;
  /** Modifier byte the report carries when the override fires. */
  replacementMods: number;
  /** Layers on which the override is active (bit `n` = layer `n`). */
  layerMask: number;
  /** Whether the override is active. */
  enabled: boolean;
}

/**
 * Build the OVERRIDE_SET request payload `[index, trig_lo, trig_hi, trig_mods,
 * repl_lo, repl_hi, repl_mods, layer_lo, layer_hi, enabled]`.
 */
export function encodeOverrideSetArgs(index: number, ov: KeyOverride): number[] {
  return [
    index & 0xff,
    ov.trigger & 0xff,
    (ov.trigger >> 8) & 0xff,
    ov.triggerMods & 0xff,
    ov.replacement & 0xff,
    (ov.replacement >> 8) & 0xff,
    ov.replacementMods & 0xff,
    ov.layerMask & 0xff,
    (ov.layerMask >> 8) & 0xff,
    ov.enabled ? 1 : 0,
  ];
}

/**
 * Parse an OVERRIDE_GET reply `[present, trig_lo, trig_hi, trig_mods, repl_lo,
 * repl_hi, repl_mods, layer_lo, layer_hi, enabled]`. Returns `null` for an empty
 * slot (`present == 0`).
 */
export function parseOverride(payload: Uint8Array): KeyOverride | null {
  if (payload[0] === 0) {
    return null;
  }
  return {
    trigger: readU16LE(payload, 1),
    triggerMods: payload[3],
    replacement: readU16LE(payload, 4),
    replacementMods: payload[6],
    layerMask: readU16LE(payload, 7),
    enabled: payload[9] !== 0,
  };
}

// === Capacities ============================================================

/** Behaviour table capacities (BEHAVIOR_INFO reply `[MAX_SOCD, MAX_OVERRIDES]`). */
export interface BehaviorInfo {
  /** Maximum SOCD pairs the firmware's RAM table holds (`behavior::MAX_SOCD`). */
  maxSocd: number;
  /** Maximum key overrides (`behavior::MAX_OVERRIDES`). */
  maxOverrides: number;
}

/** Parse a BEHAVIOR_INFO reply payload `[MAX_SOCD, MAX_OVERRIDES]`. */
export function parseBehaviorInfo(payload: Uint8Array): BehaviorInfo {
  return {
    maxSocd: payload[0],
    maxOverrides: payload[1],
  };
}

// === Tap-dance =============================================================

/**
 * A configured tap-dance entry (mirror of `timed::TapDanceCfg`). Keycodes are
 * raw u16; a `double` of `NONE` (0) falls back to the tap action.
 */
export interface TapDance {
  /** Action on a single tap (raw keycode). */
  tap: number;
  /** Action when held past the term (raw keycode). */
  hold: number;
  /** Action on a double tap (raw keycode; 0 = fall back to `tap`). */
  double: number;
  /** Decision window in milliseconds (`tap_term_ms`). */
  termMs: number;
}

/** Build the TAPDANCE_SET request `[index, tap, hold, double, term]` (each u16 LE). */
export function encodeTapdanceSetArgs(index: number, td: TapDance): number[] {
  return [
    index & 0xff,
    td.tap & 0xff,
    (td.tap >> 8) & 0xff,
    td.hold & 0xff,
    (td.hold >> 8) & 0xff,
    td.double & 0xff,
    (td.double >> 8) & 0xff,
    td.termMs & 0xff,
    (td.termMs >> 8) & 0xff,
  ];
}

/**
 * Parse a TAPDANCE_GET reply `[present, tap, hold, double, term]` (keycodes
 * u16 LE). Returns `null` for an empty slot (`present == 0`).
 */
export function parseTapdance(payload: Uint8Array): TapDance | null {
  if (payload[0] === 0) {
    return null;
  }
  return {
    tap: readU16LE(payload, 1),
    hold: readU16LE(payload, 3),
    double: readU16LE(payload, 5),
    termMs: readU16LE(payload, 7),
  };
}

// === Combos ================================================================

/** Smallest combo key-set the firmware accepts (`timed::MIN_COMBO_KEYS`). */
export const MIN_COMBO_KEYS = 2;

/**
 * Per-combo flag bits — mirror of `timed::ComboCfg`'s `FLAG_*` (the same byte the
 * config blob stores). Must-hold and must-tap are mutually exclusive (the firmware
 * rejects both set together).
 */
export const COMBO_FLAG_MUST_HOLD = 1 << 0;
export const COMBO_FLAG_MUST_TAP = 1 << 1;
export const COMBO_FLAG_IN_ORDER = 1 << 2;

/**
 * A configured combo (mirror of `timed::ComboCfg`). `keys` holds the active
 * member keycodes (`MIN_COMBO_KEYS..=MAX_COMBO_KEYS` of them); `len` on the wire
 * is `keys.length`. Keycodes are raw u16.
 */
export interface Combo {
  /** The member keycodes that must be chorded (raw keycodes), 2..=4 of them. */
  keys: number[];
  /** Action emitted when the chord fires (raw keycode). */
  action: number;
  /** Recognition window in milliseconds (`term_ms`). */
  termMs: number;
  /** Must-hold: fire only once the chord is held for the term, else type individually. */
  mustHold: boolean;
  /** Must-tap: fire only when the chord is tapped (released within the term). */
  mustTap: boolean;
  /** In-order: the members must be pressed in the listed order. */
  inOrder: boolean;
}

/** Pack a combo's flag booleans into the wire flags byte. */
function comboFlagsByte(combo: Combo): number {
  return (
    (combo.mustHold ? COMBO_FLAG_MUST_HOLD : 0) |
    (combo.mustTap ? COMBO_FLAG_MUST_TAP : 0) |
    (combo.inOrder ? COMBO_FLAG_IN_ORDER : 0)
  );
}

/**
 * Build the COMBO_SET request `[index, len, k0, k1, k2, k3, action, term, flags]`.
 * The four key slots are always sent; `len` (= `combo.keys.length`) tells the
 * firmware how many are active and the unused trailing slots are zero-padded.
 */
export function encodeComboSetArgs(index: number, combo: Combo): number[] {
  const keys = [0, 0, 0, 0];
  for (let i = 0; i < combo.keys.length && i < keys.length; i += 1) {
    keys[i] = combo.keys[i] & 0xffff;
  }
  const args = [index & 0xff, combo.keys.length & 0xff];
  for (const key of keys) {
    args.push(key & 0xff, (key >> 8) & 0xff);
  }
  args.push(combo.action & 0xff, (combo.action >> 8) & 0xff);
  args.push(combo.termMs & 0xff, (combo.termMs >> 8) & 0xff);
  args.push(comboFlagsByte(combo));
  return args;
}

/**
 * Parse a COMBO_GET reply `[present, len, k0..k3, action, term, flags]`. The `keys`
 * array is trimmed to the active `len`. Returns `null` for an empty slot.
 */
export function parseCombo(payload: Uint8Array): Combo | null {
  if (payload[0] === 0) {
    return null;
  }
  const len = payload[1];
  const keys: number[] = [];
  for (let i = 0; i < len; i += 1) {
    keys.push(readU16LE(payload, 2 + i * 2));
  }
  const flags = payload[14];
  return {
    keys,
    action: readU16LE(payload, 10),
    termMs: readU16LE(payload, 12),
    mustHold: (flags & COMBO_FLAG_MUST_HOLD) !== 0,
    mustTap: (flags & COMBO_FLAG_MUST_TAP) !== 0,
    inOrder: (flags & COMBO_FLAG_IN_ORDER) !== 0,
  };
}

// === Timed-engine capacities ===============================================

/** Timed-engine table capacities (TIMED_INFO reply). */
export interface TimedInfo {
  /** Maximum tap-dance entries (`timed::MAX_TAP_DANCE`). */
  maxTapDance: number;
  /** Maximum combos (`timed::MAX_COMBO`). */
  maxCombo: number;
  /** Maximum keys per combo (`timed::MAX_COMBO_KEYS`). */
  maxComboKeys: number;
  /** Maximum macros (`timed::MAX_MACRO`). */
  maxMacro: number;
  /** Maximum steps per macro (`timed::MAX_MACRO_STEPS`). */
  maxMacroSteps: number;
  /** Maximum leader-sequence entries (`timed::MAX_LEADER`). */
  maxLeader: number;
  /** Maximum key presses per leader sequence (`timed::MAX_LEADER_SEQ`). */
  maxLeaderSeq: number;
}

/**
 * Parse a TIMED_INFO reply payload `[MAX_TAP_DANCE, MAX_COMBO, MAX_COMBO_KEYS,
 * MAX_MACRO, MAX_MACRO_STEPS, MAX_LEADER, MAX_LEADER_SEQ]`.
 */
export function parseTimedInfo(payload: Uint8Array): TimedInfo {
  return {
    maxTapDance: payload[0],
    maxCombo: payload[1],
    maxComboKeys: payload[2],
    maxMacro: payload[3],
    maxMacroSteps: payload[4],
    maxLeader: payload[5],
    maxLeaderSeq: payload[6],
  };
}

// === Leader key ============================================================

/**
 * Sequence slots a `LEADER_SET` request always carries on the wire
 * (`timed::MAX_LEADER_SEQ`); only the first `len` are active and the rest are
 * zero-padded, exactly as the combo encoder pads its four key slots.
 */
export const LEADER_SEQ_SLOTS = 5;

/**
 * A configured leader entry (mirror of `timed::LeaderCfg`). `seq` holds the active
 * sequence keycodes (`1..=MAX_LEADER_SEQ` of them); `len` on the wire is
 * `seq.length`. The `action` keycode fires when the whole sequence matches — a
 * `MACRO(n)` keycode triggers that macro, anything else is tapped. Keycodes are raw u16.
 */
export interface Leader {
  /** The keycodes pressed in order after `LEADER` (raw keycodes), 1..=5 of them. */
  seq: number[];
  /** Action emitted when the sequence matches (raw keycode). */
  action: number;
}

/**
 * Build the LEADER_SET request `[index, len, s0..s4, action]`. The five sequence
 * slots are always sent; `len` (= `leader.seq.length`) tells the firmware how many
 * are active and the unused trailing slots are zero-padded. A `len` of `0` clears
 * the slot.
 */
export function encodeLeaderSetArgs(index: number, leader: Leader): number[] {
  const seq = new Array<number>(LEADER_SEQ_SLOTS).fill(0);
  for (let i = 0; i < leader.seq.length && i < seq.length; i += 1) {
    seq[i] = leader.seq[i] & 0xffff;
  }
  const args = [index & 0xff, leader.seq.length & 0xff];
  for (const key of seq) {
    args.push(key & 0xff, (key >> 8) & 0xff);
  }
  args.push(leader.action & 0xff, (leader.action >> 8) & 0xff);
  return args;
}

/**
 * Parse a LEADER_GET reply `[len, s0..s4, action]`. The `seq` array is trimmed to
 * the active `len`. Returns `null` for an empty slot (`len == 0`).
 */
export function parseLeader(payload: Uint8Array): Leader | null {
  const len = payload[0];
  if (len === 0) {
    return null;
  }
  const seq: number[] = [];
  for (let i = 0; i < len; i += 1) {
    seq.push(readU16LE(payload, 1 + i * 2));
  }
  return {
    seq,
    action: readU16LE(payload, 1 + LEADER_SEQ_SLOTS * 2),
  };
}
