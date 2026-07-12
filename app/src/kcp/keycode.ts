// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Keycode model — a faithful TypeScript mirror of the firmware's 16-bit keycode
 * encoding (`firmware/src/keycode.rs`). The kcp KEYMAP group carries
 * keycodes as opaque little-endian `u16`s, so the GUI decodes/encodes them with
 * exactly the firmware's partitioning of the 16-bit space:
 *
 * | Range             | Meaning                                                                                                     |
 * |-------------------|-------------------------------------------------------------------------------------------------------------|
 * | `0x0000`          | None — no-op (unbound key)                                                                                  |
 * | `0x0001`          | Transparent — transparent; fall through to a lower layer                                                    |
 * | `0x0004..=0x00DF` | basic HID keyboard usage (page 0x07)                                                                        |
 * | `0x00E0..=0x00E7` | the eight modifiers (`LCtrl`..`RGui`), page 0x07                                                            |
 * | `0x00E8..=0x00FF` | remaining basic HID usages                                                                                  |
 * | `0x2000..=0x3FFF` | `MT(mods,kc)` — mod-tap: hold `mods`, tap `kc`                                                              |
 * | `0x4000..=0x4FFF` | `LT(layer,kc)` — layer-tap: hold `layer`, tap `kc`                                                          |
 * | `0x5200..=0x521F` | `MO(n)` — momentary layer switch to layer `n` (0..=31)                                                      |
 * | `0x5300..=0x531F` | `TO(n)` — activate layer `n`, others off (0..=31)                                                           |
 * | `0x5400..=0x541F` | `TG(n)` — toggle layer `n` (0..=31)                                                                         |
 * | `0x5500..=0x55FF` | mouse keys (move / buttons / wheel)                                                                         |
 * | `0x5600..=0x56FF` | gamepad keys (16 buttons / 4 axes)                                                                          |
 * | `0x5700..=0x57FF` | `TD(n)` — tap-dance entry `n` (0..=255)                                                                     |
 * | `0x5800..=0x581F` | `TT(n)` — tap = toggle / hold = momentary (0..=31)                                                          |
 * | `0x5900..=0x591F` | `OSL(n)` — one-shot layer `n` until next key (0..=31)                                                       |
 * | `0x5A00..=0x5A08` | firmware behaviour controls (layer-lock / auto-shift / leader / caps-word / key-lock / repeat / alt-repeat) |
 * | `0x5A09..=0x5A10` | `OSM(mod)` — one-shot modifier; HID modifier bit index in low bits                                          |
 * | `0x5A20..=0x5A2F` | `DF(n)` — set the persistent default layer (0..=15)                                                         |
 * | `0x5A30`          | `GRAVE_ESCAPE` — grave-escape (Esc, or grave/`~` under Shift/GUI)                                           |
 * | `0x5A31..=0x5A33` | Space-Cadet shift keys (paren / Enter on tap, modifier on hold)                                             |
 * | `0x5A40..=0x5A42` | autocorrect controls (`AUTOCORRECT_TOGGLE` / `AUTOCORRECT_ON` / `AUTOCORRECT_OFF`)                          |
 * | `0x5A50`          | `UNICODE_MODE_CYCLE` — cycle the active OS Unicode input mode                                               |
 * | `0x5A51..=0x5A60` | `UM(n)` — emit Unicode-map slot `n`'s codepoint (0..=15)                                                    |
 * | `0x7700..=0x77FF` | `MACRO(n)` — dynamic-macro entry `n` (0..=255)                                                              |
 * | `0x7C00`          | `BOOTLOADER` — reset into the wb32-dfu bootloader                                                           |
 * | `0xC000..=0xFFFF` | consumer-control usage (page 0x0C), 14-bit usage                                                            |
 * | everything else   | unassigned, decoded as None                                                                                 |
 *
 * Decoding is {@link classify}; encoding the inverse {@link encodeAction}. The
 * two round-trip on every canonical encoding, exactly like `Keycode::classify`
 * and `Keycode::raw` in the firmware.
 */

// === Encoding regions (mirror of the `keycode.rs` consts) ==================

/** Base of the momentary-layer (`MO`) region. */
export const MO_BASE = 0x5200;
/**
 * Mask selecting the layer index out of an `MO` keycode (layers 0..=31). The
 * `TO`/`TG`/`TT`/`OSL` regions reuse it — they encode a layer the same way.
 */
export const MO_LAYER_MASK = 0x001f;
/** Base of the activate-layer (`TO`) region (`0x5300..=0x531F`). */
export const TO_BASE = 0x5300;
/** Base of the toggle-layer (`TG`) region (`0x5400..=0x541F`). */
export const TG_BASE = 0x5400;
/** Base of the tap-toggle-layer (`TT`) region (`0x5800..=0x581F`). */
export const TT_BASE = 0x5800;
/** Base of the one-shot-layer (`OSL`) region (`0x5900..=0x591F`). */
export const OSL_BASE = 0x5900;
/** Base of the mod-tap (`MT`) region (`0x2000..=0x3FFF`): low byte = tap usage, bits 8..=12 = 5-bit mod selector. */
export const MOD_TAP_BASE = 0x2000;
/** Inclusive top of the `MT` region (`0x3FFF`). */
export const MOD_TAP_TOP = 0x3fff;
/** Base of the layer-tap (`LT`) region (`0x4000..=0x4FFF`): low byte = tap usage, bits 8..=11 = 4-bit layer. */
export const LAYER_TAP_BASE = 0x4000;
/** Inclusive top of the `LT` region (`0x4FFF`). */
export const LAYER_TAP_TOP = 0x4fff;
/** Mask selecting the tap usage out of an `MT`/`LT` keycode (the low byte). */
export const TAP_HOLD_KC_MASK = 0x00ff;
/** Bit offset of the modifier/layer selector within an `MT`/`LT` keycode. */
export const TAP_HOLD_SEL_SHIFT = 8;
/** Mask selecting the 5-bit modifier selector out of an `MT` keycode (after the shift). */
export const MOD_TAP_SEL_MASK = 0x1f;
/** Mask selecting the 4-bit layer out of an `LT` keycode (after the shift), layers 0..=15. */
export const LAYER_TAP_LAYER_MASK = 0x0f;
/** First HID usage that denotes a modifier (`LCtrl`); the eight run to `0xE7`. */
export const MOD_USAGE_LO = 0x00e0;
/** Last HID usage that denotes a modifier (`RGui`). */
export const MOD_USAGE_HI = 0x00e7;
/** Base of the consumer-control region (`0xC000..=0xFFFF`). */
export const CONSUMER_BASE = 0xc000;
/** Mask selecting the 14-bit consumer usage out of a consumer keycode. */
export const CONSUMER_USAGE_MASK = 0x3fff;
/** Base of the tap-dance (`TD`) region (`0x5700..=0x57FF`); the low byte is the entry. */
export const TAP_DANCE_BASE = 0x5700;
/** Base of the dynamic-macro (`MACRO`) region (`0x7700..=0x77FF`); the low byte is the entry. */
export const MACRO_BASE = 0x7700;
/** Mask selecting the entry index out of a `TD`/`MACRO` keycode (entries 0..=255). */
export const ENTRY_INDEX_MASK = 0x00ff;
/** Base of the mouse-key region (`0x5500..=0x55FF`); the low byte selects the action. */
export const MOUSE_BASE = 0x5500;
/** Number of assigned mouse keys — codes `0..=8` (move, buttons, wheel). */
export const MOUSE_KEY_COUNT = 9;
/** Base of the gamepad-key region (`0x5600..=0x56FF`); the low byte selects the action. */
export const GAMEPAD_BASE = 0x5600;
/**
 * Number of assigned gamepad keys — codes `0..=23`: sixteen buttons (`0..=15`) then
 * the eight axis-direction keys (`16..=23`), the two signed ends of X, Y, Z and Rz.
 */
export const GAMEPAD_KEY_COUNT = 24;
/** The bootloader-entry keycode (`0x7C00`): resets into the wb32-dfu bootloader. */
export const BOOTLOADER = 0x7c00;

/**
 * Base of the firmware behaviour-control region (`0x5A00..=0x5A08`): the
 * parameterless control codes the keymap / timed engines and the behaviour plugins act
 * on directly. The one-shot-modifier codes (`OSM`) follow at `0x5A09..=0x5A10`.
 */
export const BEHAVIOR_KC_BASE = 0x5a00;
/** Layer-lock key (`LAYER_LOCK`): locks the highest active layer on (press again to unlock). */
export const LAYER_LOCK = 0x5a00;
/** Auto-shift toggle (`AUTO_SHIFT_TOGGLE`): flips the auto-shift enable flag. */
export const AUTO_SHIFT_TOGGLE = 0x5a01;
/** Auto-shift on (`AUTO_SHIFT_ON`). */
export const AUTO_SHIFT_ON = 0x5a02;
/** Auto-shift off (`AUTO_SHIFT_OFF`). */
export const AUTO_SHIFT_OFF = 0x5a03;
/** Leader key (`LEADER`): opens a leader sequence the next keys are matched against. */
export const LEADER = 0x5a04;
/** Caps-word key (`CAPS_WORD`): holds Shift across a word, ended by a non-word key. */
export const CAPS_WORD = 0x5a05;
/** Key-lock key (`KEY_LOCK`): latches the next key down until it is pressed again. */
export const KEY_LOCK = 0x5a06;
/** Repeat key (`REPEAT`): re-emits the last emitted key and its modifiers. */
export const REPEAT = 0x5a07;
/** Alternate-repeat key (`ALT_REPEAT`): re-emits the last key's mapped alternate. */
export const ALT_REPEAT = 0x5a08;
/** Number of assigned parameterless behaviour-control codes (`0x5A00..=0x5A08`). */
export const BEHAVIOR_KC_COUNT = 9;
/**
 * Base of the one-shot-modifier (`OSM`) region (`0x5A09..=0x5A10`): the HID modifier bit
 * index (`0..=7`) is the offset from this base, one code per modifier (`LCtrl..RGui`).
 */
export const OSM_BASE = 0x5a09;
/** Number of one-shot-modifier codes — one per HID modifier bit (`0..=7`). */
export const OSM_MOD_COUNT = 8;

/**
 * Base of the default-layer (`DF`) region (`0x5A20..`): the offset from the base is the
 * target base layer. The region holds one code per keymap layer (the firmware's sixteen
 * layers), matched in {@link classify} up to {@link DEFAULT_LAYER_COUNT}.
 */
export const DEFAULT_LAYER_BASE = 0x5a20;
/** Number of assigned default-layer codes — one per keymap layer (mirrors the firmware's `LAYERS`). */
export const DEFAULT_LAYER_COUNT = 16;
/** Grave-escape key (`GRAVE_ESCAPE`, `0x5A30`): Escape, or grave/`~` while Shift or GUI is held. */
export const GRAVE_ESCAPE = 0x5a30;
/** Base of the Space-Cadet region (`0x5A31..=0x5A33`). */
export const SPACE_CADET_BASE = 0x5a31;
/** Space-Cadet: Left Shift held, `(` on tap (`SPACE_CADET_PAREN_LEFT`). */
export const SPACE_CADET_PAREN_LEFT = 0x5a31;
/** Space-Cadet: Right Shift held, `)` on tap (`SPACE_CADET_PAREN_RIGHT`). */
export const SPACE_CADET_PAREN_RIGHT = 0x5a32;
/** Space-Cadet: Right Shift held, Enter on tap (`SPACE_CADET_ENTER`). */
export const SPACE_CADET_ENTER = 0x5a33;
/** Number of assigned Space-Cadet codes (`0x5A31..=0x5A33`). */
export const SPACE_CADET_COUNT = 3;

/**
 * Base of the autocorrect control region (`0x5A40..=0x5A42`): the offset from the base is
 * the {@link AutocorrectAction} (toggle / on / off), matched in {@link classify} up to
 * {@link AUTOCORRECT_KC_COUNT}.
 */
export const AUTOCORRECT_KC_BASE = 0x5a40;
/** Autocorrect toggle (`AUTOCORRECT_TOGGLE`, `0x5A40`): flips the autocorrect enable flag. */
export const AUTOCORRECT_TOGGLE = 0x5a40;
/** Autocorrect on (`AUTOCORRECT_ON`, `0x5A41`). */
export const AUTOCORRECT_ON = 0x5a41;
/** Autocorrect off (`AUTOCORRECT_OFF`, `0x5A42`). */
export const AUTOCORRECT_OFF = 0x5a42;
/** Number of assigned autocorrect control codes (`0x5A40..=0x5A42`). */
export const AUTOCORRECT_KC_COUNT = 3;

/** Unicode mode-cycle key (`UNICODE_MODE_CYCLE`, `0x5A50`): cycles the active OS input mode. */
export const UNICODE_MODE_CYCLE = 0x5a50;
/**
 * Base of the Unicode-map (`UM`) region (`0x5A51..=0x5A60`): the offset from the base is
 * the codepoint slot the sender emits, one code per host-uploaded codepoint.
 */
export const UNICODE_MAP_BASE = 0x5a51;
/** Number of Unicode-map slots (`UM(0)..=UM(15)`). */
export const UNICODE_MAP_COUNT = 16;

/** The two engine sentinels. */
export const NONE = 0x0000;
export const TRANSPARENT = 0x0001;

// === Decoded action (mirror of `enum KeyAction`) ===========================

/** Discriminant of a decoded keycode. */
export type KeyActionKind =
  | 'noop'
  | 'transparent'
  | 'key'
  | 'modifier'
  | 'momentary'
  | 'to'
  | 'tg'
  | 'tt'
  | 'osl'
  | 'modtap'
  | 'layertap'
  | 'consumer'
  | 'tapdance'
  | 'macro'
  | 'mouse'
  | 'gamepad'
  | 'boot'
  | 'layerlock'
  | 'autoshift'
  | 'leader'
  | 'capsword'
  | 'keylock'
  | 'repeat'
  | 'altrepeat'
  | 'oneshotmod'
  | 'defaultlayer'
  | 'gesc'
  | 'spacecadet'
  | 'autocorrect'
  | 'unicodecycle'
  | 'unicodemap';

/** The runtime control an auto-shift keycode performs (mirror of `AutoShiftAction`). */
export type AutoShiftAction = 'toggle' | 'on' | 'off';

/** The runtime control an autocorrect keycode performs (mirror of `AutocorrectAction`). */
export type AutocorrectAction = 'toggle' | 'on' | 'off';

/** The role a Space-Cadet key performs (mirror of the firmware's `SpaceCadet`). */
export type SpaceCadetRole = 'lspo' | 'rspc' | 'sent';

/** A decoded keycode, the TS equivalent of the firmware's `KeyAction`. */
export type KeyAction =
  | { kind: 'noop' }
  | { kind: 'transparent' }
  /** A basic key, carrying its HID keyboard usage (page 0x07). */
  | { kind: 'key'; usage: number }
  /** A modifier, carrying its bit index 0..=7 within the HID modifier byte. */
  | { kind: 'modifier'; index: number }
  /** Momentary layer switch, carrying the target layer index. */
  | { kind: 'momentary'; layer: number }
  /** Activate-layer switch (`TO`): makes the target layer the only active non-base one. */
  | { kind: 'to'; layer: number }
  /** Toggle-layer switch (`TG`): each press flips whether the layer is latched. */
  | { kind: 'tg'; layer: number }
  /** Tap-or-hold layer switch (`TT`): momentary held, toggles on a bare tap. */
  | { kind: 'tt'; layer: number }
  /** One-shot layer switch (`OSL`): active until the next key press. */
  | { kind: 'osl'; layer: number }
  /** Mod-tap (`MT`): tap emits `kc`, hold asserts `mods` (the HID modifier byte). */
  | { kind: 'modtap'; mods: number; kc: number }
  /** Layer-tap (`LT`): tap emits `kc`, hold momentarily activates `layer`. */
  | { kind: 'layertap'; layer: number; kc: number }
  /** A consumer-control key, carrying its 14-bit HID consumer usage (page 0x0C). */
  | { kind: 'consumer'; usage: number }
  /** Tap-dance key, carrying its entry index into the timed engine's table. */
  | { kind: 'tapdance'; index: number }
  /** Dynamic-macro key, carrying its entry index into the timed engine's table. */
  | { kind: 'macro'; index: number }
  /** Mouse key, carrying its action code 0..=8 (the firmware `MouseKey` discriminant). */
  | { kind: 'mouse'; code: number }
  /**
   * Gamepad key, carrying its action code 0..=23 (the firmware `GamepadKey` region
   * offset): 0..=15 are the buttons, 16..=23 the eight axis-direction keys.
   */
  | { kind: 'gamepad'; code: number }
  /** The bootloader-entry key, `BOOTLOADER`: jumps into the wb32-dfu bootloader. */
  | { kind: 'boot' }
  /** Layer-lock key (`LAYER_LOCK`): locks the highest active layer on, or unlocks it. */
  | { kind: 'layerlock' }
  /** Auto-shift control key, carrying the toggle / on / off action it performs. */
  | { kind: 'autoshift'; action: AutoShiftAction }
  /** Leader key (`LEADER`): opens a leader sequence. */
  | { kind: 'leader' }
  /** Caps-word key (`CAPS_WORD`): holds Shift across a word until a non-word key. */
  | { kind: 'capsword' }
  /** Key-lock key (`KEY_LOCK`): latches the next key down until it is pressed again. */
  | { kind: 'keylock' }
  /** Repeat key (`REPEAT`): re-emits the last emitted key and its modifiers. */
  | { kind: 'repeat' }
  /** Alternate-repeat key (`ALT_REPEAT`): re-emits the last key's mapped alternate. */
  | { kind: 'altrepeat' }
  /** One-shot-modifier key (`OSM`), carrying the HID modifier bit index (`0..=7`). */
  | { kind: 'oneshotmod'; index: number }
  /** Default-layer key (`DF(n)`): makes the carried layer the persistent base. */
  | { kind: 'defaultlayer'; layer: number }
  /** Grave-escape key (`GRAVE_ESCAPE`): Escape, or grave/`~` while Shift or GUI is held. */
  | { kind: 'gesc' }
  /** Space-Cadet shift key: a paren / Enter on tap, the modifier on hold. */
  | { kind: 'spacecadet'; role: SpaceCadetRole }
  /** Autocorrect control key, carrying the toggle / on / off action it performs. */
  | { kind: 'autocorrect'; action: AutocorrectAction }
  /** Unicode mode-cycle key (`UNICODE_MODE_CYCLE`): cycles the active OS input mode. */
  | { kind: 'unicodecycle' }
  /** Unicode-map key (`UM(n)`), carrying the codepoint slot index (`0..=15`). */
  | { kind: 'unicodemap'; index: number };

/**
 * Expand an `MT` 5-bit modifier selector into the HID modifier byte — the inverse
 * of {@link compressMods}, mirroring the firmware's `expand_mods`.
 */
function expandMods(sel: number): number {
  const nibble = sel & 0x0f;
  return sel & 0x10 ? nibble << 4 : nibble;
}

/**
 * Compress a single-sided HID modifier byte into the `MT` 5-bit selector, mirroring
 * the firmware's `compress_mods`. A byte with any right-hand modifier (`0xF0`) encodes
 * right-side; a mixed-side byte is not representable and resolves right-side.
 */
function compressMods(mods: number): number {
  const m = mods & 0xff;
  return m & 0xf0 ? 0x10 | ((m >> 4) & 0x0f) : m & 0x0f;
}

/**
 * Decode a raw `u16` into its {@link KeyAction}. Total, like
 * `Keycode::classify`: any value out of the assigned regions decodes to `noop`,
 * the same safe fall-back the firmware uses for an unbound key.
 */
export function classify(raw: number): KeyAction {
  const r = raw & 0xffff;
  if (r === NONE) return { kind: 'noop' };
  if (r === TRANSPARENT) return { kind: 'transparent' };
  // Modifiers (0xE0..=0xE7) are matched before the basic-key ranges, exactly
  // as the firmware orders its arms.
  if (r >= MOD_USAGE_LO && r <= MOD_USAGE_HI) return { kind: 'modifier', index: r - MOD_USAGE_LO };
  if ((r >= 0x0004 && r <= 0x00df) || (r >= 0x00e8 && r <= 0x00ff))
    return { kind: 'key', usage: r };
  // Mod-tap / layer-tap sit below `MO` in the otherwise-unassigned span, matched here
  // as the firmware does.
  if (r >= MOD_TAP_BASE && r <= MOD_TAP_TOP)
    return {
      kind: 'modtap',
      mods: expandMods((r >> TAP_HOLD_SEL_SHIFT) & MOD_TAP_SEL_MASK),
      kc: r & TAP_HOLD_KC_MASK,
    };
  if (r >= LAYER_TAP_BASE && r <= LAYER_TAP_TOP)
    return {
      kind: 'layertap',
      layer: (r >> TAP_HOLD_SEL_SHIFT) & LAYER_TAP_LAYER_MASK,
      kc: r & TAP_HOLD_KC_MASK,
    };
  if (r >= MO_BASE && r <= MO_BASE + MO_LAYER_MASK)
    return { kind: 'momentary', layer: r & MO_LAYER_MASK };
  if (r >= TO_BASE && r <= TO_BASE + MO_LAYER_MASK) return { kind: 'to', layer: r & MO_LAYER_MASK };
  if (r >= TG_BASE && r <= TG_BASE + MO_LAYER_MASK) return { kind: 'tg', layer: r & MO_LAYER_MASK };
  // Only the assigned codes (0..=8) decode to a mouse key; the rest of the reserved
  // `0x5500..=0x55FF` region falls through to the NoOp default, mirroring the firmware.
  if (r >= MOUSE_BASE && r < MOUSE_BASE + MOUSE_KEY_COUNT)
    return { kind: 'mouse', code: r - MOUSE_BASE };
  // Only the assigned codes (0..=23) decode to a gamepad key; the rest of the
  // reserved `0x5600..=0x56FF` region falls through to NoOp, mirroring the firmware.
  if (r >= GAMEPAD_BASE && r < GAMEPAD_BASE + GAMEPAD_KEY_COUNT)
    return { kind: 'gamepad', code: r - GAMEPAD_BASE };
  if (r >= TAP_DANCE_BASE && r <= TAP_DANCE_BASE + ENTRY_INDEX_MASK)
    return { kind: 'tapdance', index: r & ENTRY_INDEX_MASK };
  if (r >= TT_BASE && r <= TT_BASE + MO_LAYER_MASK) return { kind: 'tt', layer: r & MO_LAYER_MASK };
  if (r >= OSL_BASE && r <= OSL_BASE + MO_LAYER_MASK)
    return { kind: 'osl', layer: r & MO_LAYER_MASK };
  // Behaviour-control codes (0x5A00..=0x5A08); the OSM codes follow, and the rest of
  // the reserved region falls through to NoOp, mirroring the firmware.
  if (r >= BEHAVIOR_KC_BASE && r < BEHAVIOR_KC_BASE + BEHAVIOR_KC_COUNT) {
    switch (r - BEHAVIOR_KC_BASE) {
      case 0:
        return { kind: 'layerlock' };
      case 1:
        return { kind: 'autoshift', action: 'toggle' };
      case 2:
        return { kind: 'autoshift', action: 'on' };
      case 3:
        return { kind: 'autoshift', action: 'off' };
      case 4:
        return { kind: 'leader' };
      case 5:
        return { kind: 'capsword' };
      case 6:
        return { kind: 'keylock' };
      case 7:
        return { kind: 'repeat' };
      default:
        return { kind: 'altrepeat' };
    }
  }
  if (r >= OSM_BASE && r < OSM_BASE + OSM_MOD_COUNT)
    return { kind: 'oneshotmod', index: r - OSM_BASE };
  // Default-layer / grave-escape / Space-Cadet sub-block (`0x5A20..=0x5A3F`); the gaps
  // fall through to NoOp, mirroring the firmware.
  if (r >= DEFAULT_LAYER_BASE && r < DEFAULT_LAYER_BASE + DEFAULT_LAYER_COUNT)
    return { kind: 'defaultlayer', layer: r - DEFAULT_LAYER_BASE };
  if (r === GRAVE_ESCAPE) return { kind: 'gesc' };
  if (r >= SPACE_CADET_BASE && r < SPACE_CADET_BASE + SPACE_CADET_COUNT) {
    const role: SpaceCadetRole =
      r === SPACE_CADET_PAREN_LEFT ? 'lspo' : r === SPACE_CADET_PAREN_RIGHT ? 'rspc' : 'sent';
    return { kind: 'spacecadet', role };
  }
  // Autocorrect controls (`0x5A40..=0x5A42`); the rest of the block falls through to
  // NoOp, mirroring the firmware.
  if (r >= AUTOCORRECT_KC_BASE && r < AUTOCORRECT_KC_BASE + AUTOCORRECT_KC_COUNT) {
    const action: AutocorrectAction =
      r === AUTOCORRECT_TOGGLE ? 'toggle' : r === AUTOCORRECT_ON ? 'on' : 'off';
    return { kind: 'autocorrect', action };
  }
  // Unicode sub-block (`0x5A50..=0x5A60`): the mode-cycle code then the `UM` map.
  if (r === UNICODE_MODE_CYCLE) return { kind: 'unicodecycle' };
  if (r >= UNICODE_MAP_BASE && r < UNICODE_MAP_BASE + UNICODE_MAP_COUNT)
    return { kind: 'unicodemap', index: r - UNICODE_MAP_BASE };
  if (r >= MACRO_BASE && r <= MACRO_BASE + ENTRY_INDEX_MASK)
    return { kind: 'macro', index: r & ENTRY_INDEX_MASK };
  if (r === BOOTLOADER) return { kind: 'boot' };
  if (r >= CONSUMER_BASE && r <= 0xffff)
    return { kind: 'consumer', usage: r & CONSUMER_USAGE_MASK };
  return { kind: 'noop' };
}

/**
 * Encode a {@link KeyAction} back into a raw `u16` — the inverse of
 * {@link classify} on every canonical encoding. Mirrors the firmware
 * constructors (`from_usage`, `momentary`, `consumer`) and their masking.
 */
export function encodeAction(action: KeyAction): number {
  switch (action.kind) {
    case 'noop':
      return NONE;
    case 'transparent':
      return TRANSPARENT;
    case 'key':
      return action.usage & 0xffff;
    case 'modifier':
      return (MOD_USAGE_LO + (action.index & 0x07)) & 0xffff;
    case 'momentary':
      return MO_BASE | (action.layer & MO_LAYER_MASK);
    case 'to':
      return TO_BASE | (action.layer & MO_LAYER_MASK);
    case 'tg':
      return TG_BASE | (action.layer & MO_LAYER_MASK);
    case 'tt':
      return TT_BASE | (action.layer & MO_LAYER_MASK);
    case 'osl':
      return OSL_BASE | (action.layer & MO_LAYER_MASK);
    case 'modtap':
      return (
        MOD_TAP_BASE |
        (compressMods(action.mods) << TAP_HOLD_SEL_SHIFT) |
        (action.kc & TAP_HOLD_KC_MASK)
      );
    case 'layertap':
      return (
        LAYER_TAP_BASE |
        ((action.layer & LAYER_TAP_LAYER_MASK) << TAP_HOLD_SEL_SHIFT) |
        (action.kc & TAP_HOLD_KC_MASK)
      );
    case 'consumer':
      return CONSUMER_BASE | (action.usage & CONSUMER_USAGE_MASK);
    case 'tapdance':
      return TAP_DANCE_BASE | (action.index & ENTRY_INDEX_MASK);
    case 'macro':
      return MACRO_BASE | (action.index & ENTRY_INDEX_MASK);
    case 'mouse':
      return MOUSE_BASE | (action.code & 0xff);
    case 'gamepad':
      return GAMEPAD_BASE | (action.code & 0xff);
    case 'boot':
      return BOOTLOADER;
    case 'layerlock':
      return LAYER_LOCK;
    case 'autoshift':
      return BEHAVIOR_KC_BASE + (action.action === 'toggle' ? 1 : action.action === 'on' ? 2 : 3);
    case 'leader':
      return LEADER;
    case 'capsword':
      return CAPS_WORD;
    case 'keylock':
      return KEY_LOCK;
    case 'repeat':
      return REPEAT;
    case 'altrepeat':
      return ALT_REPEAT;
    case 'oneshotmod':
      return OSM_BASE + (action.index & 0x07);
    case 'defaultlayer':
      return DEFAULT_LAYER_BASE + (action.layer % DEFAULT_LAYER_COUNT);
    case 'gesc':
      return GRAVE_ESCAPE;
    case 'spacecadet':
      return action.role === 'lspo'
        ? SPACE_CADET_PAREN_LEFT
        : action.role === 'rspc'
          ? SPACE_CADET_PAREN_RIGHT
          : SPACE_CADET_ENTER;
    case 'autocorrect':
      return action.action === 'toggle'
        ? AUTOCORRECT_TOGGLE
        : action.action === 'on'
          ? AUTOCORRECT_ON
          : AUTOCORRECT_OFF;
    case 'unicodecycle':
      return UNICODE_MODE_CYCLE;
    case 'unicodemap':
      return UNICODE_MAP_BASE + (action.index % UNICODE_MAP_COUNT);
  }
}

/** Build a keycode from a raw basic HID usage (`Keycode::from_usage`). */
export function fromUsage(usage: number): number {
  return usage & 0xffff;
}

/** Build a momentary layer switch, `MO(layer)` (`Keycode::momentary`). */
export function momentary(layer: number): number {
  return MO_BASE | (layer & MO_LAYER_MASK);
}

/** Build an activate-layer switch, `TO(layer)` (`Keycode::to_layer`). */
export function toLayer(layer: number): number {
  return TO_BASE | (layer & MO_LAYER_MASK);
}

/** Build a toggle-layer switch, `TG(layer)` (`Keycode::toggle`). */
export function toggle(layer: number): number {
  return TG_BASE | (layer & MO_LAYER_MASK);
}

/** Build a tap-or-hold layer switch, `TT(layer)` (`Keycode::tap_toggle`). */
export function tapToggle(layer: number): number {
  return TT_BASE | (layer & MO_LAYER_MASK);
}

/** Build a one-shot layer switch, `OSL(layer)` (`Keycode::one_shot`). */
export function oneShot(layer: number): number {
  return OSL_BASE | (layer & MO_LAYER_MASK);
}

/** Build a default-layer key, `DF(layer)` (`Keycode::default_layer`). */
export function defaultLayer(layer: number): number {
  return DEFAULT_LAYER_BASE + (layer % DEFAULT_LAYER_COUNT);
}

/**
 * Build a mod-tap, `MT(mods, kc)` (`Keycode::mod_tap`): tap emits the basic usage
 * `kc`, hold asserts `mods` (a single-sided HID modifier byte — see {@link compressMods}).
 */
export function modTap(mods: number, kc: number): number {
  return MOD_TAP_BASE | (compressMods(mods) << TAP_HOLD_SEL_SHIFT) | (kc & TAP_HOLD_KC_MASK);
}

/**
 * Build a layer-tap, `LT(layer, kc)` (`Keycode::layer_tap`): tap emits the basic usage
 * `kc`, hold momentarily activates `layer` (`0..=15`).
 */
export function layerTap(layer: number, kc: number): number {
  return (
    LAYER_TAP_BASE |
    ((layer & LAYER_TAP_LAYER_MASK) << TAP_HOLD_SEL_SHIFT) |
    (kc & TAP_HOLD_KC_MASK)
  );
}

/**
 * The eight HID modifiers, by bit, for the mod-tap builder and labels. The bit is the
 * modifier's position in the HID modifier byte (`LCtrl` = `0x01` … `RGui` = `0x80`); a
 * mod-tap can hold a single-sided combination, but the picker offers them singly (the
 * common home-row-mod case).
 */
export const MODIFIERS: ReadonlyArray<{ bit: number; short: string; token: string; name: string }> =
  [
    { bit: 0x01, short: 'LCtrl', token: 'ControlLeft', name: 'Left Control' },
    { bit: 0x02, short: 'LShift', token: 'ShiftLeft', name: 'Left Shift' },
    { bit: 0x04, short: 'LAlt', token: 'AltLeft', name: 'Left Alt' },
    { bit: 0x08, short: 'LCmd', token: 'MetaLeft', name: 'Left GUI' },
    { bit: 0x10, short: 'RCtrl', token: 'ControlRight', name: 'Right Control' },
    { bit: 0x20, short: 'RShift', token: 'ShiftRight', name: 'Right Shift' },
    { bit: 0x40, short: 'RAlt', token: 'AltRight', name: 'Right Alt' },
    { bit: 0x80, short: 'RCmd', token: 'MetaRight', name: 'Right GUI' },
  ];

/**
 * Canonical W3C-token label for a HID modifier byte, e.g. `ControlLeft` or
 * `ControlLeft+ShiftLeft`; `None` when empty. Used to spell the modifier argument of the
 * mod-tap / one-shot-mod tokens.
 */
function modsToken(mods: number): string {
  const parts = MODIFIERS.filter((m) => (mods & m.bit) !== 0).map((m) => m.token);
  return parts.length ? parts.join('+') : 'None';
}

/** Descriptive label for a HID modifier byte, e.g. `Left Control`; `no modifier` when empty. */
function modsLongLabel(mods: number): string {
  const parts = MODIFIERS.filter((m) => (mods & m.bit) !== 0).map((m) => m.name);
  return parts.length ? parts.join(' + ') : 'no modifier';
}

/** Build a consumer-control keycode from a raw HID consumer usage. */
export function consumer(usage: number): number {
  return CONSUMER_BASE | (usage & CONSUMER_USAGE_MASK);
}

/** Build a tap-dance keycode, `TD(index)`, naming an entry of the timed table. */
export function tapdance(index: number): number {
  return TAP_DANCE_BASE | (index & ENTRY_INDEX_MASK);
}

/** Build a dynamic-macro keycode, `MACRO(index)`, naming an entry of the macro table. */
export function macro(index: number): number {
  return MACRO_BASE | (index & ENTRY_INDEX_MASK);
}

/** Build a mouse keycode from its action code 0..=8 (the firmware `MouseKey`). */
export function mouse(code: number): number {
  return MOUSE_BASE | (code & 0xff);
}

/**
 * Build a gamepad keycode from its action code 0..=23 (the firmware `GamepadKey`
 * region offset): 0..=15 are the buttons, 16..=23 the eight axis-direction keys.
 */
export function gamepad(code: number): number {
  return GAMEPAD_BASE | (code & 0xff);
}

/** Build an auto-shift control keycode (`Keycode::auto_shift`). */
export function autoShift(action: AutoShiftAction): number {
  return BEHAVIOR_KC_BASE + (action === 'toggle' ? 1 : action === 'on' ? 2 : 3);
}

/** Build an autocorrect control keycode (`Keycode::autocorrect`). */
export function autocorrect(action: AutocorrectAction): number {
  return action === 'toggle'
    ? AUTOCORRECT_TOGGLE
    : action === 'on'
      ? AUTOCORRECT_ON
      : AUTOCORRECT_OFF;
}

/** Build the caps-word keycode, `CAPS_WORD` (`Keycode::caps_word`). */
export function capsWord(): number {
  return CAPS_WORD;
}

/** Build the key-lock keycode, `KEY_LOCK` (`Keycode::key_lock`). */
export function keyLock(): number {
  return KEY_LOCK;
}

/** Build the repeat keycode, `REPEAT` (`Keycode::repeat`). */
export function repeatKey(): number {
  return REPEAT;
}

/** Build the alternate-repeat keycode, `ALT_REPEAT` (`Keycode::alt_repeat`). */
export function altRepeat(): number {
  return ALT_REPEAT;
}

/** Build a one-shot-modifier keycode, `OSM(bit)`, naming the HID modifier bit index `0..=7`. */
export function oneShotMod(bit: number): number {
  return OSM_BASE + (bit & 0x07);
}

/** Build the Unicode mode-cycle keycode, `UNICODE_MODE_CYCLE` (`Keycode::unicode_cycle`). */
export function unicodeModeCycle(): number {
  return UNICODE_MODE_CYCLE;
}

/** Build a Unicode-map keycode, `UM(slot)`, naming the codepoint slot `0..=15` (`Keycode::unicode_map`). */
export function unicodeMap(slot: number): number {
  return UNICODE_MAP_BASE + (((slot % UNICODE_MAP_COUNT) + UNICODE_MAP_COUNT) % UNICODE_MAP_COUNT);
}

// === Named keycode catalogue (mirror of the firmware keycode constants) ====

/** Picker categories — the firmware's keycode groupings, GUI-facing. */
export type KeycodeCategory =
  | 'special'
  | 'letters'
  | 'numbers'
  | 'whitespace'
  | 'symbols'
  | 'function'
  | 'nav'
  | 'modifiers'
  | 'media'
  | 'mouse'
  | 'gamepad';

/** A named keycode, for the picker and for labelling keys in the grid. */
export interface NamedKeycode {
  /** Raw 16-bit encoding. */
  raw: number;
  /**
   * Canonical W3C `KeyboardEvent.code`-style token (PascalCase, side as a `Left`/`Right`
   * suffix), e.g. `KeyA`, `ArrowUp`, `ControlLeft`, `AudioVolumeUp`.
   */
  token: string;
  /** Short cap label (fits a key in the grid), e.g. `A`, `Esc`, `→`. */
  label: string;
  /** Descriptive name for the picker, e.g. `Right Arrow`. */
  name: string;
  /** Picker category. */
  category: KeycodeCategory;
}

/** Human-readable labels for the picker categories, in display order. */
export const CATEGORY_LABELS: ReadonlyArray<{ category: KeycodeCategory; label: string }> = [
  { category: 'special', label: 'Special' },
  { category: 'letters', label: 'Letters' },
  { category: 'numbers', label: 'Numbers' },
  { category: 'whitespace', label: 'Whitespace' },
  { category: 'symbols', label: 'Symbols' },
  { category: 'function', label: 'F-keys' },
  { category: 'nav', label: 'Navigation' },
  { category: 'modifiers', label: 'Modifiers' },
  { category: 'media', label: 'Media' },
  { category: 'mouse', label: 'Mouse' },
  { category: 'gamepad', label: 'Gamepad' },
];

function key(
  raw: number,
  token: string,
  label: string,
  name: string,
  category: KeycodeCategory,
): NamedKeycode {
  return { raw, token, label, name, category };
}

const LETTERS: NamedKeycode[] = Array.from({ length: 26 }, (_, i) => {
  const letter = String.fromCharCode(65 + i); // A..Z
  return key(fromUsage(0x04 + i), `Key${letter}`, letter, letter, 'letters');
});

const NUMBERS: NamedKeycode[] = ['1', '2', '3', '4', '5', '6', '7', '8', '9', '0'].map((d, i) =>
  key(fromUsage(0x1e + i), `Digit${d}`, d, d, 'numbers'),
);

const FUNCTION: NamedKeycode[] = Array.from({ length: 12 }, (_, i) =>
  key(fromUsage(0x3a + i), `F${i + 1}`, `F${i + 1}`, `F${i + 1}`, 'function'),
);

// Gamepad buttons: code `i` (0..=15) is the firmware `GamepadKey::Button(i)`, sent as
// HID Button `i + 1` — so the token/label/name use the 1-indexed host number.
const GAMEPAD_BUTTONS: NamedKeycode[] = Array.from({ length: 16 }, (_, i) =>
  key(gamepad(i), `GamepadButton${i + 1}`, `Pad ${i + 1}`, `Gamepad Button ${i + 1}`, 'gamepad'),
);

/**
 * The full named catalogue. Letters, numbers and F-keys are generated; the rest
 * are spelled out against their firmware HID usages so the labels are exact.
 */
export const KEYCODES: readonly NamedKeycode[] = [
  // Special
  key(NONE, 'None', 'None', 'No-op (unbound)', 'special'),
  key(TRANSPARENT, 'Transparent', 'Trans', 'Transparent (fall through)', 'special'),

  ...LETTERS,
  ...NUMBERS,

  // Whitespace / editing
  key(fromUsage(0x28), 'Enter', 'Enter', 'Enter', 'whitespace'),
  key(fromUsage(0x29), 'Escape', 'Esc', 'Escape', 'whitespace'),
  key(fromUsage(0x2a), 'Backspace', 'Bksp', 'Backspace', 'whitespace'),
  key(fromUsage(0x2b), 'Tab', 'Tab', 'Tab', 'whitespace'),
  key(fromUsage(0x2c), 'Space', 'Space', 'Space', 'whitespace'),

  // Symbols
  key(fromUsage(0x2d), 'Minus', '-', 'Minus', 'symbols'),
  key(fromUsage(0x2e), 'Equal', '=', 'Equal', 'symbols'),
  key(fromUsage(0x2f), 'BracketLeft', '[', 'Left Bracket', 'symbols'),
  key(fromUsage(0x30), 'BracketRight', ']', 'Right Bracket', 'symbols'),
  key(fromUsage(0x31), 'Backslash', '\\', 'Backslash', 'symbols'),
  key(fromUsage(0x32), 'IntlHash', '#~', 'Non-US # / ~', 'symbols'),
  key(fromUsage(0x33), 'Semicolon', ';', 'Semicolon', 'symbols'),
  key(fromUsage(0x34), 'Quote', "'", 'Quote', 'symbols'),
  key(fromUsage(0x35), 'Backquote', '`', 'Backquote (Grave)', 'symbols'),
  key(fromUsage(0x36), 'Comma', ',', 'Comma', 'symbols'),
  key(fromUsage(0x37), 'Period', '.', 'Period', 'symbols'),
  key(fromUsage(0x38), 'Slash', '/', 'Slash', 'symbols'),
  key(fromUsage(0x39), 'CapsLock', 'Caps', 'Caps Lock', 'symbols'),
  key(fromUsage(0x64), 'IntlBackslash', '\\|', 'Non-US \\ / |', 'symbols'),
  key(fromUsage(0x65), 'ContextMenu', 'Menu', 'Context Menu', 'symbols'),

  ...FUNCTION,

  // Navigation / system
  key(fromUsage(0x46), 'PrintScreen', 'PrtSc', 'Print Screen', 'nav'),
  key(fromUsage(0x47), 'ScrollLock', 'ScrLk', 'Scroll Lock', 'nav'),
  key(fromUsage(0x48), 'Pause', 'Pause', 'Pause', 'nav'),
  key(fromUsage(0x49), 'Insert', 'Ins', 'Insert', 'nav'),
  key(fromUsage(0x4a), 'Home', 'Home', 'Home', 'nav'),
  key(fromUsage(0x4b), 'PageUp', 'PgUp', 'Page Up', 'nav'),
  key(fromUsage(0x4c), 'Delete', 'Del', 'Delete (forward)', 'nav'),
  key(fromUsage(0x4d), 'End', 'End', 'End', 'nav'),
  key(fromUsage(0x4e), 'PageDown', 'PgDn', 'Page Down', 'nav'),
  key(fromUsage(0x4f), 'ArrowRight', '→', 'Right Arrow', 'nav'),
  key(fromUsage(0x50), 'ArrowLeft', '←', 'Left Arrow', 'nav'),
  key(fromUsage(0x51), 'ArrowDown', '↓', 'Down Arrow', 'nav'),
  key(fromUsage(0x52), 'ArrowUp', '↑', 'Up Arrow', 'nav'),

  // Modifiers (HID usages 0xE0..=0xE7) — W3C side-suffix tokens
  key(fromUsage(0xe0), 'ControlLeft', 'LCtrl', 'Left Control', 'modifiers'),
  key(fromUsage(0xe1), 'ShiftLeft', 'LShift', 'Left Shift', 'modifiers'),
  key(fromUsage(0xe2), 'AltLeft', 'LAlt', 'Left Alt / Option', 'modifiers'),
  key(fromUsage(0xe3), 'MetaLeft', 'LCmd', 'Left Meta (Command / Win / Super)', 'modifiers'),
  key(fromUsage(0xe4), 'ControlRight', 'RCtrl', 'Right Control', 'modifiers'),
  key(fromUsage(0xe5), 'ShiftRight', 'RShift', 'Right Shift', 'modifiers'),
  key(fromUsage(0xe6), 'AltRight', 'RAlt', 'Right Alt / AltGr', 'modifiers'),
  key(fromUsage(0xe7), 'MetaRight', 'RCmd', 'Right Meta (Command / Win / Super)', 'modifiers'),

  // Consumer-control (media / volume / brightness / launchers, page 0x0C)
  key(consumer(0xe2), 'AudioVolumeMute', 'Mute', 'Mute', 'media'),
  key(consumer(0xe9), 'AudioVolumeUp', 'Vol +', 'Volume Up', 'media'),
  key(consumer(0xea), 'AudioVolumeDown', 'Vol -', 'Volume Down', 'media'),
  key(consumer(0xcd), 'MediaPlayPause', 'Play', 'Play / Pause', 'media'),
  key(consumer(0xb5), 'MediaTrackNext', 'Next', 'Next Track', 'media'),
  key(consumer(0xb6), 'MediaTrackPrevious', 'Prev', 'Previous Track', 'media'),
  key(consumer(0xb7), 'MediaStop', 'Stop', 'Stop', 'media'),
  key(consumer(0x6f), 'BrightnessUp', 'Bright +', 'Brightness Up', 'media'),
  key(consumer(0x70), 'BrightnessDown', 'Bright -', 'Brightness Down', 'media'),
  key(consumer(0x192), 'LaunchApp2', 'Calc', 'Calculator', 'media'),
  key(consumer(0x194), 'LaunchApp1', 'Files', 'My Computer', 'media'),
  key(consumer(0x18a), 'LaunchMail', 'Mail', 'Mail', 'media'),
  key(consumer(0x221), 'BrowserSearch', 'Search', 'Web Search', 'media'),
  key(consumer(0x223), 'BrowserHome', 'Web', 'Web Home', 'media'),
  key(consumer(0x183), 'MediaSelect', 'Media', 'Media Select', 'media'),

  // Mouse keys (move / buttons / wheel, the 0x5500 region)
  key(mouse(0), 'MouseUp', 'Mouse ↑', 'Mouse Up', 'mouse'),
  key(mouse(1), 'MouseDown', 'Mouse ↓', 'Mouse Down', 'mouse'),
  key(mouse(2), 'MouseLeft', 'Mouse ←', 'Mouse Left', 'mouse'),
  key(mouse(3), 'MouseRight', 'Mouse →', 'Mouse Right', 'mouse'),
  key(mouse(4), 'MouseButton1', 'LMB', 'Mouse Button 1 (Left)', 'mouse'),
  key(mouse(5), 'MouseButton2', 'RMB', 'Mouse Button 2 (Right)', 'mouse'),
  key(mouse(6), 'MouseButton3', 'MMB', 'Mouse Button 3 (Middle)', 'mouse'),
  key(mouse(7), 'MouseWheelUp', 'Wheel ↑', 'Wheel Up', 'mouse'),
  key(mouse(8), 'MouseWheelDown', 'Wheel ↓', 'Wheel Down', 'mouse'),

  // Gamepad keys (16 buttons + 4 signed axes, the 0x5600 region)
  ...GAMEPAD_BUTTONS,
  key(gamepad(16), 'JoystickXMinus', 'X -', 'Joystick X-', 'gamepad'),
  key(gamepad(17), 'JoystickXPlus', 'X +', 'Joystick X+', 'gamepad'),
  key(gamepad(18), 'JoystickYMinus', 'Y -', 'Joystick Y-', 'gamepad'),
  key(gamepad(19), 'JoystickYPlus', 'Y +', 'Joystick Y+', 'gamepad'),
  key(gamepad(20), 'JoystickZMinus', 'Z -', 'Joystick Z-', 'gamepad'),
  key(gamepad(21), 'JoystickZPlus', 'Z +', 'Joystick Z+', 'gamepad'),
  key(gamepad(22), 'JoystickRzMinus', 'Rz -', 'Joystick Rz-', 'gamepad'),
  key(gamepad(23), 'JoystickRzPlus', 'Rz +', 'Joystick Rz+', 'gamepad'),
];

/** Catalogue indexed by raw code, for label lookups. */
const BY_RAW: ReadonlyMap<number, NamedKeycode> = new Map(KEYCODES.map((k) => [k.raw, k]));

/** The named keycodes of one category, in catalogue order. */
export function keycodesByCategory(category: KeycodeCategory): NamedKeycode[] {
  return KEYCODES.filter((k) => k.category === category);
}

/**
 * Short cap label for any raw keycode — the catalogue label when known, else the
 * canonical {@link keycodeToken} for the layer/behaviour codes (`MomentaryLayer(2)`,
 * `LayerLock`), `CC 0x…` for an unlisted consumer usage and `0x…` for an unlisted
 * basic usage.
 */
export function keycodeLabel(raw: number): string {
  const named = BY_RAW.get(raw & 0xffff);
  if (named) return named.label;
  const action = classify(raw);
  switch (action.kind) {
    case 'consumer':
      return `CC 0x${action.usage.toString(16).toUpperCase()}`;
    case 'key':
      return `0x${action.usage.toString(16).toUpperCase().padStart(2, '0')}`;
    default:
      return keycodeToken(raw);
  }
}

/**
 * Canonical W3C `KeyboardEvent.code` / KKN token for any raw keycode — the catalogue
 * {@link NamedKeycode.token} when listed, else derived from the decoded action:
 * `MomentaryLayer(2)`, `ModTap(ControlLeft, KeyA)`, `OneShotMod(ShiftLeft)`, `LayerLock`,
 * … . Derived purely from `raw` (like {@link keycodeLabel} / {@link keycodeName}), so it
 * never desyncs from the encoding.
 */
export function keycodeToken(raw: number): string {
  const named = BY_RAW.get(raw & 0xffff);
  if (named) return named.token;
  const action = classify(raw);
  switch (action.kind) {
    case 'noop':
      return 'None';
    case 'transparent':
      return 'Transparent';
    case 'momentary':
      return `MomentaryLayer(${action.layer})`;
    case 'to':
      return `ActivateLayer(${action.layer})`;
    case 'tg':
      return `ToggleLayer(${action.layer})`;
    case 'tt':
      return `TapToggleLayer(${action.layer})`;
    case 'osl':
      return `OneShotLayer(${action.layer})`;
    case 'modtap':
      return `ModTap(${modsToken(action.mods)}, ${keycodeToken(action.kc)})`;
    case 'layertap':
      return `LayerTap(${action.layer}, ${keycodeToken(action.kc)})`;
    case 'tapdance':
      return `TapDance(${action.index})`;
    case 'macro':
      return `Macro(${action.index})`;
    case 'boot':
      return 'Bootloader';
    case 'layerlock':
      return 'LayerLock';
    case 'autoshift':
      return action.action === 'toggle'
        ? 'AutoShiftToggle'
        : action.action === 'on'
          ? 'AutoShiftOn'
          : 'AutoShiftOff';
    case 'leader':
      return 'Leader';
    case 'capsword':
      return 'CapsWord';
    case 'keylock':
      return 'KeyLock';
    case 'repeat':
      return 'Repeat';
    case 'altrepeat':
      return 'AltRepeat';
    case 'oneshotmod':
      return `OneShotMod(${modsToken(1 << action.index)})`;
    case 'defaultlayer':
      return `DefaultLayer(${action.layer})`;
    case 'gesc':
      return 'GraveEscape';
    case 'spacecadet':
      return action.role === 'lspo'
        ? 'SpaceCadetParenLeft'
        : action.role === 'rspc'
          ? 'SpaceCadetParenRight'
          : 'SpaceCadetEnter';
    case 'autocorrect':
      return action.action === 'toggle'
        ? 'AutocorrectToggle'
        : action.action === 'on'
          ? 'AutocorrectOn'
          : 'AutocorrectOff';
    case 'unicodecycle':
      return 'UnicodeModeCycle';
    case 'unicodemap':
      return `UnicodeMap(${action.index})`;
    case 'consumer':
      return `Consumer(0x${action.usage.toString(16).toUpperCase()})`;
    case 'key':
      return `0x${action.usage.toString(16).toUpperCase().padStart(2, '0')}`;
    case 'mouse':
      // Every mouse key is in the catalogue, so this is unreachable in practice.
      return `Mouse${action.code}`;
    case 'gamepad':
      // Every gamepad key is in the catalogue, so this is unreachable in practice.
      return `Pad${action.code}`;
    case 'modifier':
      // Every modifier is in the catalogue, so this is unreachable in practice.
      return `Mod${action.index}`;
  }
}

/** Descriptive name for any raw keycode (catalogue name, else the cap label). */
export function keycodeName(raw: number): string {
  const named = BY_RAW.get(raw & 0xffff);
  if (named) return named.name;
  // The behaviour-control codes have no catalogue entry (like Boot), so spell out
  // their descriptive names here for the picker tooltip and the grid.
  const action = classify(raw);
  switch (action.kind) {
    case 'layerlock':
      return 'Layer Lock';
    case 'autoshift':
      return action.action === 'toggle'
        ? 'Auto-Shift Toggle'
        : action.action === 'on'
          ? 'Auto-Shift On'
          : 'Auto-Shift Off';
    case 'leader':
      return 'Leader Key';
    case 'capsword':
      return 'Caps Word';
    case 'keylock':
      return 'Key Lock';
    case 'repeat':
      return 'Repeat Key';
    case 'altrepeat':
      return 'Alternate Repeat Key';
    case 'oneshotmod':
      return `One-Shot ${modsLongLabel(1 << action.index)}`;
    case 'defaultlayer':
      return `Default Layer ${action.layer}`;
    case 'gesc':
      return 'Grave-Escape (Esc / ` / ~)';
    case 'spacecadet':
      return action.role === 'lspo'
        ? 'Space-Cadet: Left Shift / ('
        : action.role === 'rspc'
          ? 'Space-Cadet: Right Shift / )'
          : 'Space-Cadet: Right Shift / Enter';
    case 'autocorrect':
      return action.action === 'toggle'
        ? 'Autocorrect Toggle'
        : action.action === 'on'
          ? 'Autocorrect On'
          : 'Autocorrect Off';
    case 'modtap':
      return `Mod-Tap: hold ${modsLongLabel(action.mods)}, tap ${keycodeName(action.kc)}`;
    case 'layertap':
      return `Layer-Tap: hold layer ${action.layer}, tap ${keycodeName(action.kc)}`;
    case 'unicodecycle':
      return 'Unicode: cycle OS input mode';
    case 'unicodemap':
      return `Unicode Map ${action.index}`;
    default:
      return keycodeLabel(raw);
  }
}
