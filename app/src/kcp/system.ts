// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * SYSTEM group (0xFx) — MCU-level resets. Mirrors `system_dispatch` in
 * `firmware/src/kcp.rs` byte-for-byte.
 *
 * Both operations reset the chip, so — unlike every other group — the firmware
 * never sends a reply: `handle` resets before it can. The host therefore issues
 * the command fire-and-forget and treats the ensuing USB disconnect as the
 * acknowledgement (exactly as QMK's `QK_BOOT` is used). There is consequently no
 * request payload, no reply payload and nothing to parse — only the two op codes
 * (in `protocol.ts`) and the reset semantics:
 *
 *  - ENTER_DFU (0xF0): reset into the `wb32-dfu` ROM bootloader for re-flashing.
 *  - REBOOT    (0xF1): reboot the firmware normally.
 */

/** A fire-and-forget request sender: writes the OUT report, awaiting no reply. */
export type ResetSender = (cmd: number) => Promise<void>;

/**
 * Issue a SYSTEM reset command (ENTER_DFU / REBOOT). The MCU resets before it can
 * reply, dropping off USB, so we never wait for a reply. A send that rejects
 * because the pipe tore down mid-reset is that same expected disconnect, so it is
 * treated as success rather than surfaced as an error — the disconnect *is* the
 * acknowledgement.
 */
export async function issueReset(send: ResetSender, cmd: number): Promise<void> {
  try {
    await send(cmd);
  } catch {
    // The reset dropped the USB pipe mid-send — the acknowledgement, not a fault.
  }
}

/**
 * The USB device personality (`SYSTEM.SET_USB_MODE` / `GET_USB_MODE`). Mirrors
 * `enum UsbMode` in `firmware/src/usb.rs`. `Midi` and `XInput` each re-enumerate the
 * device as a single-purpose class (still exposing the kcp control interface, so the
 * switch is reversible); `Normal` is the full keeberry composite.
 */
export enum UsbMode {
  Normal = 0,
  Midi = 1,
  // An Xbox 360 (XInput) controller (firmware code 2): an active re-enumerated mode,
  // first-class on Windows and Linux. macOS has no native XInput driver, so there the
  // always-on HID gamepad (the gamepad/joystick keycodes) is the gamepad path instead.
  XInput = 2,
}

const USB_MODE_LABELS: Record<UsbMode, string> = {
  [UsbMode.Normal]: 'Normal (keyboard)',
  [UsbMode.Midi]: 'MIDI',
  [UsbMode.XInput]: 'XInput (gamepad)',
};

/** Human-readable label for a {@link UsbMode} (handles unknown values). */
export function usbModeLabel(mode: number): string {
  return USB_MODE_LABELS[mode as UsbMode] ?? `Unknown mode (${mode})`;
}

/**
 * Decode the `SYSTEM.GET_USB_MODE` reply payload (`[mode]`). An unknown code (a
 * firmware newer than this client) throws, so the caller never acts on a personality
 * it cannot represent.
 */
export function parseUsbMode(payload: Uint8Array): UsbMode {
  const mode = payload[0];
  if (mode in USB_MODE_LABELS) {
    return mode as UsbMode;
  }
  throw new Error(`unknown USB mode code ${mode}`);
}

/** Inclusive maximum of the digitizer's absolute X/Y range (mirror of `usb.rs`). */
export const DIGITIZER_LOGICAL_MAX = 0x7fff;

/**
 * Encode the `SYSTEM.SET_DIGITIZER` request payload `[flags, x_lo, x_hi, y_lo,
 * y_hi]`. `x`/`y` are clamped to `0..=`{@link DIGITIZER_LOGICAL_MAX} and sent
 * little-endian; `flags` packs the tip switch (bit 0) and in-range (bit 1).
 */
export function encodeDigitizerArgs(
  x: number,
  y: number,
  tip: boolean,
  inRange: boolean,
): number[] {
  const clamp = (v: number) => Math.max(0, Math.min(DIGITIZER_LOGICAL_MAX, Math.round(v)));
  const cx = clamp(x);
  const cy = clamp(y);
  const flags = (tip ? 0x01 : 0) | (inRange ? 0x02 : 0);
  return [flags, cx & 0xff, cx >> 8, cy & 0xff, cy >> 8];
}
