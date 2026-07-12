// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { encodeSetKroArgs, parseKro } from './hidkro';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import { createFakeDevice, fakeFirmwareHandle } from './firmware-fixture';

describe('parseKro / encodeSetKroArgs (mirror of hid_kro_dispatch)', () => {
  it('reads the NKRO flag from byte 0', () => {
    expect(parseKro(new Uint8Array([1]))).toBe(true);
    expect(parseKro(new Uint8Array([0]))).toBe(false);
  });

  it('encodes the SET_KRO argument as a single 0/1 byte', () => {
    expect(encodeSetKroArgs(true)).toEqual([1]);
    expect(encodeSetKroArgs(false)).toEqual([0]);
  });
});

describe('HID_KRO dispatch through the codec (set/get is live)', () => {
  it('boots NKRO off and observes a SET_KRO on the next GET_KRO', () => {
    const device = createFakeDevice();
    expect(
      parseKro(decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.GetKro, 0), device)).payload),
    ).toBe(false);

    const set = decodeReply(
      fakeFirmwareHandle(encodeRequest(Cmd.SetKro, 1, encodeSetKroArgs(true)), device),
    );
    expect(set.status).toBe(Status.Ok);
    expect(
      parseKro(decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.GetKro, 2), device)).payload),
    ).toBe(true);
  });

  it('rejects an out-of-range SET_KRO argument with BadArg', () => {
    const device = createFakeDevice();
    expect(decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.SetKro, 1, [2]), device)).status).toBe(
      Status.BadArg,
    );
  });
});
