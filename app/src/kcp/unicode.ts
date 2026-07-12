// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * UNICODE group (0xAx) wire helpers. Mirrors `firmware/src/features/unicode.rs` and the
 * `CMD_UNICODE_*` opcodes in `kcp.rs` byte-for-byte.
 *
 * The firmware types a codepoint through the host OS's own input method, so each mode is
 * just a key sequence the firmware plays back (no codepoint→glyph table on-device):
 *  - Linux (IBus): Ctrl+Shift+U, the hex digits, then Space.
 *  - macOS (Unicode Hex Input): hold Option, type the UTF-16 hex, release Option.
 *  - Windows (WinCompose): tap Right Alt, the hex digits, then Enter.
 *
 * The codepoint map is RAM-only on the device (no flash persistence), so the host
 * re-uploads it on every connect.
 *
 * Ops (low nibble of CMD):
 *  - GET (0xA0): no request payload; reply `[activeMode, slotCount, modeCount]`.
 *  - SET_MODE (0xA1): request `[mode]`; an out-of-range mode answers BadArg.
 *  - SET_MAP (0xA2): request `[slot, cp(4, LE u32)]`; an out-of-range slot answers
 *    BadArg. A `0` codepoint clears the slot (it then types nothing).
 */

/** Host-uploadable codepoint slots (`UM(0)..=UM(15)`), mirror of `UNICODE_MAP_SLOTS`. */
export const UNICODE_MAP_SLOTS = 16;

/**
 * The OS input mode the sender targets (mirror of the firmware's mode constants). Each
 * names the OS Unicode entry method the firmware plays back.
 */
export enum UnicodeMode {
  /** Linux IBus: Ctrl+Shift+U, hex, Space. */
  Linux = 0,
  /** macOS Unicode Hex Input: hold Option, type the UTF-16 hex, release. */
  MacOS = 1,
  /** Windows WinCompose: tap Right Alt, hex, Enter. */
  Windows = 2,
}

const UNICODE_MODE_LABELS: Record<UnicodeMode, string> = {
  [UnicodeMode.Linux]: 'Linux (IBus)',
  [UnicodeMode.MacOS]: 'macOS (Unicode Hex Input)',
  [UnicodeMode.Windows]: 'Windows (WinCompose)',
};

/** Human-readable label for a {@link UnicodeMode} (handles unknown values). */
export function unicodeModeLabel(mode: number): string {
  return UNICODE_MODE_LABELS[mode as UnicodeMode] ?? `Unknown mode (${mode})`;
}

/** The Unicode-input state reported by GET (0xA0). */
export interface UnicodeInfo {
  /** Active OS input mode. */
  mode: UnicodeMode;
  /** Number of codepoint slots the firmware exposes. */
  slots: number;
  /** Number of OS modes the firmware can target. */
  modeCount: number;
}

/** Parse a GET reply payload `[activeMode, slotCount, modeCount]`. */
export function parseUnicodeInfo(payload: Uint8Array): UnicodeInfo {
  return { mode: payload[0] as UnicodeMode, slots: payload[1], modeCount: payload[2] };
}

/** Build the SET_MODE request payload `[mode]`. */
export function encodeSetModeArgs(mode: UnicodeMode): number[] {
  return [mode & 0xff];
}

/** Build the SET_MAP request payload `[slot, cp(4, LE u32)]`. */
export function encodeSetMapArgs(slot: number, codepoint: number): number[] {
  const cp = codepoint >>> 0;
  return [slot & 0xff, cp & 0xff, (cp >>> 8) & 0xff, (cp >>> 16) & 0xff, (cp >>> 24) & 0xff];
}
