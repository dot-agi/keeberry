// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { encodeSetAutocorrectArgs, parseAutocorrectInfo } from './autocorrect';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  AUTOCORRECT_ENTRY_COUNT,
  createFakeDevice,
  fakeFirmwareHandle,
} from './firmware-fixture';

describe('parseAutocorrectInfo / encodeSetAutocorrectArgs (mirror of the autocorrect dispatch)', () => {
  it('reads the enable flag from byte 0 and the entry count as a u16 LE', () => {
    expect(parseAutocorrectInfo(new Uint8Array([1, 0x35, 0x00]))).toEqual({
      enabled: true,
      entryCount: 0x35,
    });
    expect(parseAutocorrectInfo(new Uint8Array([0, 0x00, 0x01]))).toEqual({
      enabled: false,
      entryCount: 0x100,
    });
  });

  it('encodes the AUTOCORRECT_SET argument as a single 0/1 byte', () => {
    expect(encodeSetAutocorrectArgs(true)).toEqual([1]);
    expect(encodeSetAutocorrectArgs(false)).toEqual([0]);
  });
});

describe('TEXT autocorrect dispatch through the codec (set/get is live)', () => {
  it('boots autocorrect on, reporting the compiled-in dictionary size', () => {
    const device = createFakeDevice();
    const info = parseAutocorrectInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TextAutocorrectInfo, 0), device)).payload,
    );
    expect(info).toEqual({ enabled: true, entryCount: AUTOCORRECT_ENTRY_COUNT });
  });

  it('observes an AUTOCORRECT_SET on the next AUTOCORRECT_INFO', () => {
    const device = createFakeDevice();
    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.TextAutocorrectSet, 1, encodeSetAutocorrectArgs(false)), device),
    );
    expect(set.status).toBe(Status.Ok);
    const info = parseAutocorrectInfo(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TextAutocorrectInfo, 2), device)).payload,
    );
    expect(info.enabled).toBe(false);
  });

  it('rejects an out-of-range AUTOCORRECT_SET argument with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.TextAutocorrectSet, 1, [2]), device)).status,
    ).toBe(Status.BadArg);
  });
});
