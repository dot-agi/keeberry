// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * CONFIG group (0x4x) wire helpers. The storage descriptor mirrors `kcp.rs`'s
 * `pack_storage_info` / `config::StorageInfo` byte-for-byte.
 *
 * Ops (low nibble of CMD):
 *  - SAVE (0x40): persist the complete live config to flash (keymap, NKRO, RGB,
 *    the behaviour tables and macros); Ok or Busy (a flash write / read-back
 *    failure). No request or reply payload.
 *  - LOAD_DEFAULTS (0x41): reset the complete live config to the firmware
 *    defaults (RAM-only — a follow-up SAVE persists it). No payload.
 *  - GET_STORAGE_INFO (0x42): no request payload; reply layout below.
 *  - GET_DEBOUNCE (0x43) / SET_DEBOUNCE (0x44): read / write the matrix debounce
 *    config `[algorithm, interval]` (mirrors `matrix::DebounceAlgorithm` and the
 *    deferred-edge interval). A live edit, persisted by the next SAVE.
 *  - GET_TUNING (0x45) / SET_TUNING (0x46): read / write the timed-engine runtime
 *    tunables (auto-shift enable + timeout, leader timeout, the mod-tap / layer-tap
 *    term and flags, and the quick-tap window). A live edit, persisted by the next SAVE.
 */
import { readU16LE, readU32LE } from './bytes';

/**
 * A decoded persistence-region descriptor (`pack_storage_info`),
 * payload-relative offsets (STORAGE_INFO_LEN = 11): `0..4` region base address
 * (u32 LE), `4..8` region size in bytes (u32 LE), `8..10` stored-blob format
 * version (u16 LE; 0 when none), `10` valid flag (1 = a valid blob is stored).
 */
export interface StorageInfo {
  /** Base address of the reserved flash region. */
  base: number;
  /** Size of the reserved region, in bytes. */
  size: number;
  /** Format version of the stored config blob (0 when none / invalid). */
  version: number;
  /** Whether a valid blob (magic + version + CRC) is currently stored. */
  valid: boolean;
}

/** Parse a GET_STORAGE_INFO reply payload into a {@link StorageInfo}. */
export function parseStorageInfo(payload: Uint8Array): StorageInfo {
  return {
    base: readU32LE(payload, 0),
    size: readU32LE(payload, 4),
    version: readU16LE(payload, 8),
    valid: payload[10] !== 0,
  };
}

/**
 * Matrix debounce algorithm — mirrors `matrix::DebounceAlgorithm` in `matrix.rs`;
 * the values are the kcp `algorithm` byte.
 */
export enum DebounceAlgorithm {
  /** Symmetric deferred filter: both edges wait out the interval. Noise-resistant. */
  SymmetricDefer = 0,
  /** Eager on press, deferred on release: snappy for gaming, chatter-free release. */
  AsymmetricEager = 1,
}

/** The matrix debounce configuration (`CONFIG.GET_DEBOUNCE` / `SET_DEBOUNCE`). */
export interface DebounceConfig {
  /** The active debounce algorithm. */
  algorithm: DebounceAlgorithm;
  /** Deferred-edge interval in consecutive scans (~ms at the 1 kHz scan; `>= 1`). */
  interval: number;
}

/** Parse a GET_DEBOUNCE reply payload (`[algorithm, interval]`). */
export function parseDebounce(payload: Uint8Array): DebounceConfig {
  return { algorithm: payload[0] as DebounceAlgorithm, interval: payload[1] };
}

/** Encode the SET_DEBOUNCE request payload (`[algorithm, interval]`). */
export function encodeSetDebounceArgs(cfg: DebounceConfig): number[] {
  return [cfg.algorithm & 0xff, cfg.interval & 0xff];
}

/**
 * Bit positions of the tap-hold flavours within the TUNING flags byte — mirror of
 * `TapHoldTuning::flags_byte` in `timed.rs` (and the same byte the config blob stores).
 */
export const TH_FLAG_PERMISSIVE = 1 << 0;
export const TH_FLAG_HOLD_ON_OTHER = 1 << 1;
export const TH_FLAG_RETRO = 1 << 2;
export const TH_FLAG_CHORDAL = 1 << 3;

/**
 * The runtime tunables exchanged by `CONFIG.GET_TUNING` / `SET_TUNING` — the timed
 * engine's auto-shift, leader and mod-tap / layer-tap settings (`crate::timed`).
 * Mirrors the firmware's `[auto_shift_enabled(1), auto_shift_timeout(2 LE),
 * leader_timeout(2 LE), tap_hold_term(2 LE), tap_hold_flags(1), quick_tap_term(2 LE)]`
 * layout.
 */
export interface TuningConfig {
  /** Whether auto-shift is enabled (held keys send their shifted form). */
  autoShiftEnabled: boolean;
  /** Auto-shift hold timeout in milliseconds (`>= 1`). */
  autoShiftTimeoutMs: number;
  /** Leader inter-key timeout in milliseconds (`>= 1`). */
  leaderTimeoutMs: number;
  /** Mod-tap / layer-tap decision term in milliseconds (`>= 1`): held longer → hold. */
  tapHoldTermMs: number;
  /** Permissive hold: a nested press-and-release while held resolves to hold. */
  permissiveHold: boolean;
  /** Hold on other key press: any other key pressed while held resolves to hold at once. */
  holdOnOtherKeyPress: boolean;
  /** Retro tapping: a lone hold past the term still emits the tap on release. */
  retroTapping: boolean;
  /** Chordal hold (bilateral): a same-hand interrupt settles the tap-hold as a tap. */
  chordalHold: boolean;
  /** Quick-tap window in milliseconds (`0` = off): a re-press repeats the tap. */
  quickTapTermMs: number;
}

/**
 * Parse a GET_TUNING reply `[as_on, as_timeout(2 LE), leader_timeout(2 LE),
 * tap_hold_term(2 LE), tap_hold_flags(1), quick_tap_term(2 LE)]`.
 */
export function parseTuning(payload: Uint8Array): TuningConfig {
  const flags = payload[7];
  return {
    autoShiftEnabled: payload[0] !== 0,
    autoShiftTimeoutMs: readU16LE(payload, 1),
    leaderTimeoutMs: readU16LE(payload, 3),
    tapHoldTermMs: readU16LE(payload, 5),
    permissiveHold: (flags & TH_FLAG_PERMISSIVE) !== 0,
    holdOnOtherKeyPress: (flags & TH_FLAG_HOLD_ON_OTHER) !== 0,
    retroTapping: (flags & TH_FLAG_RETRO) !== 0,
    chordalHold: (flags & TH_FLAG_CHORDAL) !== 0,
    quickTapTermMs: readU16LE(payload, 8),
  };
}

/** Encode the SET_TUNING request payload (the same layout {@link parseTuning} reads). */
export function encodeSetTuningArgs(cfg: TuningConfig): number[] {
  const flags =
    (cfg.permissiveHold ? TH_FLAG_PERMISSIVE : 0) |
    (cfg.holdOnOtherKeyPress ? TH_FLAG_HOLD_ON_OTHER : 0) |
    (cfg.retroTapping ? TH_FLAG_RETRO : 0) |
    (cfg.chordalHold ? TH_FLAG_CHORDAL : 0);
  return [
    cfg.autoShiftEnabled ? 1 : 0,
    cfg.autoShiftTimeoutMs & 0xff,
    (cfg.autoShiftTimeoutMs >> 8) & 0xff,
    cfg.leaderTimeoutMs & 0xff,
    (cfg.leaderTimeoutMs >> 8) & 0xff,
    cfg.tapHoldTermMs & 0xff,
    (cfg.tapHoldTermMs >> 8) & 0xff,
    flags,
    cfg.quickTapTermMs & 0xff,
    (cfg.quickTapTermMs >> 8) & 0xff,
  ];
}
