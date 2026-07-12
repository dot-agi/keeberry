// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it, vi } from 'vitest';
import {
  DIGITIZER_LOGICAL_MAX,
  UsbMode,
  encodeDigitizerArgs,
  issueReset,
  parseUsbMode,
  usbModeLabel,
} from './system';
import { Cmd } from './protocol';

describe('issueReset (SYSTEM fire-and-forget reset)', () => {
  it('forwards the command to the sender', async () => {
    const send = vi.fn().mockResolvedValue(undefined);
    await issueReset(send, Cmd.SystemEnterDfu);
    expect(send).toHaveBeenCalledTimes(1);
    expect(send).toHaveBeenCalledWith(Cmd.SystemEnterDfu);
  });

  it('treats a send failure (the reset tore down USB) as success, not an error', async () => {
    const send = vi.fn().mockRejectedValue(new Error('The device was disconnected.'));
    // The disconnect is the acknowledgement, so this resolves rather than throwing.
    await expect(issueReset(send, Cmd.SystemReboot)).resolves.toBeUndefined();
    expect(send).toHaveBeenCalledWith(Cmd.SystemReboot);
  });
});

describe('parseUsbMode (SYSTEM.GET_USB_MODE reply)', () => {
  it('decodes each known personality code', () => {
    expect(parseUsbMode(Uint8Array.of(0))).toBe(UsbMode.Normal);
    expect(parseUsbMode(Uint8Array.of(1))).toBe(UsbMode.Midi);
    expect(parseUsbMode(Uint8Array.of(2))).toBe(UsbMode.XInput);
  });

  it('throws on an unknown code rather than returning a bogus mode', () => {
    expect(() => parseUsbMode(Uint8Array.of(9))).toThrow(/unknown USB mode/);
  });
});

describe('usbModeLabel', () => {
  it('labels the known modes and tolerates unknown ones', () => {
    expect(usbModeLabel(UsbMode.Normal)).toMatch(/Normal/);
    expect(usbModeLabel(UsbMode.Midi)).toBe('MIDI');
    expect(usbModeLabel(UsbMode.XInput)).toMatch(/XInput/);
    expect(usbModeLabel(42)).toMatch(/Unknown mode \(42\)/);
  });
});

describe('encodeDigitizerArgs (SYSTEM.SET_DIGITIZER request)', () => {
  it('packs flags then little-endian X/Y', () => {
    expect(encodeDigitizerArgs(0x1234, 0x0abc, true, true)).toEqual([0x03, 0x34, 0x12, 0xbc, 0x0a]);
    expect(encodeDigitizerArgs(0, 0, false, false)).toEqual([0x00, 0, 0, 0, 0]);
    // Tip only / in-range only set the matching flag bit.
    expect(encodeDigitizerArgs(0, 0, true, false)[0]).toBe(0x01);
    expect(encodeDigitizerArgs(0, 0, false, true)[0]).toBe(0x02);
  });

  it('clamps coordinates to the 0..=LOGICAL_MAX range', () => {
    expect(encodeDigitizerArgs(-50, 999999, false, true)).toEqual([
      0x02,
      0x00,
      0x00,
      DIGITIZER_LOGICAL_MAX & 0xff,
      DIGITIZER_LOGICAL_MAX >> 8,
    ]);
  });
});
