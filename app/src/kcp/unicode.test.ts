// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  UNICODE_MAP_SLOTS,
  UnicodeMode,
  encodeSetMapArgs,
  encodeSetModeArgs,
  parseUnicodeInfo,
  unicodeModeLabel,
  type UnicodeInfo,
} from './unicode';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  UNICODE_MODE_COUNT,
  createFakeDevice,
  fakeFirmwareHandle,
  packUnicodeInfo,
  type UnicodeStateSample,
} from './firmware-fixture';

describe('parseUnicodeInfo (mirror of unicode::on_kcp GET)', () => {
  it('reads [activeMode, slotCount, modeCount]', () => {
    const sample: UnicodeStateSample = { mode: UnicodeMode.MacOS, map: [] };
    const expected: UnicodeInfo = {
      mode: UnicodeMode.MacOS,
      slots: UNICODE_MAP_SLOTS,
      modeCount: UNICODE_MODE_COUNT,
    };
    expect(parseUnicodeInfo(packUnicodeInfo(sample))).toEqual(expected);
  });
});

describe('UnicodeMode helpers vs the firmware', () => {
  it('matches the firmware mode codes (Linux=0, macOS=1, Windows=2)', () => {
    expect([UnicodeMode.Linux, UnicodeMode.MacOS, UnicodeMode.Windows]).toEqual([0, 1, 2]);
    expect(UNICODE_MODE_COUNT).toBe(3);
  });

  it('labels each mode and falls back for an unknown one', () => {
    expect(unicodeModeLabel(UnicodeMode.Linux)).toBe('Linux (IBus)');
    expect(unicodeModeLabel(UnicodeMode.MacOS)).toBe('macOS (Unicode Hex Input)');
    expect(unicodeModeLabel(UnicodeMode.Windows)).toBe('Windows (WinCompose)');
    expect(unicodeModeLabel(9)).toBe('Unknown mode (9)');
  });
});

describe('encodeSetModeArgs / encodeSetMapArgs', () => {
  it('lays SET_MODE out as [mode]', () => {
    expect(encodeSetModeArgs(UnicodeMode.Windows)).toEqual([2]);
  });

  it('lays SET_MAP out as [slot, cp(4 LE)]', () => {
    // U+1F600 (😀) is 0x0001_F600 little-endian: 0x00, 0xF6, 0x01, 0x00.
    expect(encodeSetMapArgs(3, 0x1f600)).toEqual([3, 0x00, 0xf6, 0x01, 0x00]);
    // A 0 codepoint clears the slot (it then types nothing).
    expect(encodeSetMapArgs(0, 0)).toEqual([0, 0, 0, 0, 0]);
  });
});

describe('UNICODE dispatch through the codec', () => {
  it('GET reports the power-on default (Linux, 16 slots, 3 modes)', () => {
    const device = createFakeDevice();
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.UnicodeGet, 1), device));
    expect(reply.status).toBe(Status.Ok);
    expect(parseUnicodeInfo(reply.payload)).toEqual({
      mode: UnicodeMode.Linux,
      slots: UNICODE_MAP_SLOTS,
      modeCount: UNICODE_MODE_COUNT,
    });
  });

  it('GET reflects a SET_MODE', () => {
    const device = createFakeDevice();
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.UnicodeSetMode, 1, encodeSetModeArgs(UnicodeMode.MacOS)), device),
    );
    expect(set.status).toBe(Status.Ok);

    const info = parseUnicodeInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.UnicodeGet, 2), device)).payload,
    );
    expect(info.mode).toBe(UnicodeMode.MacOS);
  });

  it('SET_MAP stores the codepoint verbatim into the slot', () => {
    const device = createFakeDevice();
    const reply = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.UnicodeSetMap, 1, encodeSetMapArgs(2, 0x1f600)), device),
    );
    expect(reply.status).toBe(Status.Ok);
    expect(device.unicode.map[2]).toBe(0x1f600);
  });

  it('rejects an out-of-range mode and slot with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.UnicodeSetMode, 1, [UNICODE_MODE_COUNT]), device))
        .status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(
        fakeFirmwareHandle(encodeRequest(Cmd.UnicodeSetMap, 2, encodeSetMapArgs(UNICODE_MAP_SLOTS, 1)), device),
      ).status,
    ).toBe(Status.BadArg);
  });

  it('answers BadCmd for an unknown op in the Unicode group', () => {
    const device = createFakeDevice();
    // 0xA7 is in the UNICODE group (high nibble 0xA) but is not a defined op.
    expect(decodeReply(fakeFirmwareHandle(encodeRequest(0xa7, 1), device)).status).toBe(
      Status.BadCmd,
    );
  });
});
