// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * MACRO group (0x5x) wire helpers. A macro is a recorded sequence of key
 * press/release events with inter-step delays, replayed by the firmware's timed
 * engine. A macro is larger than one 32-byte frame, so it is transferred one
 * step per report — that *is* the chunking, keyed by `(macro, step)`. The
 * layouts mirror `kcp.rs`'s `macro_dispatch` byte-for-byte; keycodes cross the
 * wire as little-endian `u16`s, matching the KEYMAP group. The table capacities
 * (`MAX_MACRO`, `MAX_MACRO_STEPS`) are read at runtime from MACRO_INFO.
 *
 * Ops (low nibble of CMD):
 *  - INFO (0x50): no payload; reply `[MAX_MACRO, MAX_MACRO_STEPS, used(4 LE)]`,
 *    where bit `i` of `used` is set when macro `i` has at least one step.
 *  - SET_STEP (0x51): request `[macro, step, kc_lo, kc_hi, down, delay_lo,
 *    delay_hi]`; the macro grows to cover `step`. An out-of-range index is BadArg.
 *  - GET_STEP (0x52): request `[macro, step]`; reply `[present, kc_lo, kc_hi,
 *    down, delay_lo, delay_hi, len]` (`present` = 1 when `step < len`).
 *  - CLEAR (0x53): request `[macro]` (0xFF clears every macro).
 *  - PLAY (0x54): request `[macro]`; an out-of-range or empty macro is BadArg.
 *  - RECORD_START (0x55): request `[macro]`; clears the slot and captures live
 *    key edges into it (with their timing) until RECORD_STOP. Out-of-range = BadArg.
 *  - RECORD_STOP (0x56): no payload; ends recording (a no-op success if idle).
 */
import { readU16LE, readU32LE } from './bytes';

/** One macro step (mirror of `timed::MacroStep`). */
export interface MacroStep {
  /** Key or modifier the step presses or releases (raw keycode). */
  keycode: number;
  /** `true` = press (down), `false` = release (up). */
  down: boolean;
  /** Delay in milliseconds to dwell after the step, before the next one. */
  delayMs: number;
}

/** Macro table capacities and the used-slot bitmap (MACRO_INFO reply). */
export interface MacroInfo {
  /** Maximum number of macros (`timed::MAX_MACRO`). */
  maxMacro: number;
  /** Maximum steps per macro (`timed::MAX_MACRO_STEPS`). */
  maxSteps: number;
  /** Bitmap of non-empty macros: bit `i` set when macro `i` has steps. */
  used: number;
}

/** One step read back from a macro, plus the macro's active length. */
export interface MacroStepReadback {
  /** Whether this step index is within the macro's active length (`step < len`). */
  present: boolean;
  /** The macro's active step count. */
  len: number;
  /** The step's event data. */
  step: MacroStep;
}

/** Parse a MACRO_INFO reply payload `[MAX_MACRO, MAX_MACRO_STEPS, used(4 LE)]`. */
export function parseMacroInfo(payload: Uint8Array): MacroInfo {
  return {
    maxMacro: payload[0],
    maxSteps: payload[1],
    used: readU32LE(payload, 2),
  };
}

/** Build the MACRO_SET_STEP request `[macro, step, kc_lo, kc_hi, down, delay_lo, delay_hi]`. */
export function encodeMacroSetStepArgs(macro: number, step: number, ev: MacroStep): number[] {
  return [
    macro & 0xff,
    step & 0xff,
    ev.keycode & 0xff,
    (ev.keycode >> 8) & 0xff,
    ev.down ? 1 : 0,
    ev.delayMs & 0xff,
    (ev.delayMs >> 8) & 0xff,
  ];
}

/** Build the MACRO_GET_STEP request payload `[macro, step]`. */
export function encodeMacroGetStepArgs(macro: number, step: number): number[] {
  return [macro & 0xff, step & 0xff];
}

/** Build the MACRO_RECORD_START request payload `[macro]`. */
export function encodeMacroRecordStartArgs(macro: number): number[] {
  return [macro & 0xff];
}

/**
 * Parse a MACRO_GET_STEP reply `[present, kc_lo, kc_hi, down, delay_lo,
 * delay_hi, len]` into the step and the macro's active length.
 */
export function parseMacroStep(payload: Uint8Array): MacroStepReadback {
  return {
    present: payload[0] !== 0,
    len: payload[6],
    step: {
      keycode: readU16LE(payload, 1),
      down: payload[3] !== 0,
      delayMs: readU16LE(payload, 4),
    },
  };
}
