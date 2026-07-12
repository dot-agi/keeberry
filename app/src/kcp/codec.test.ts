// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { decodeReply, encodeRequest, SeqCounter } from './codec';
import { MSG_LEN, REPLY_FLAG, Status } from './protocol';
import { fakeFirmwareHandle } from './firmware-fixture';

describe('encodeRequest', () => {
  it('places CMD at [0], SEQ at [1] and the payload from [2]', () => {
    const frame = encodeRequest(0x12, 0x34, [0xaa, 0xbb, 0xcc]);
    expect(frame.length).toBe(MSG_LEN);
    expect(frame[0]).toBe(0x12);
    expect(frame[1]).toBe(0x34);
    expect([frame[2], frame[3], frame[4]]).toEqual([0xaa, 0xbb, 0xcc]);
  });

  it('zero-pads the remaining payload bytes', () => {
    const frame = encodeRequest(0x01, 0x02, [0xff]);
    expect(frame[2]).toBe(0xff);
    expect(Array.from(frame.slice(3))).toEqual(new Array(MSG_LEN - 3).fill(0));
  });

  it('masks CMD and SEQ to a byte', () => {
    const frame = encodeRequest(0x1ff, 0x2ab);
    expect(frame[0]).toBe(0xff);
    expect(frame[1]).toBe(0xab);
  });

  it('accepts a full 30-byte request payload', () => {
    const payload = Array.from({ length: 30 }, (_, i) => i);
    const frame = encodeRequest(0x00, 0x00, payload);
    expect(Array.from(frame.slice(2))).toEqual(payload);
  });

  it('throws when the payload exceeds the 30-byte request region', () => {
    expect(() => encodeRequest(0x00, 0x00, new Array(31).fill(0))).toThrow(RangeError);
  });
});

describe('decodeReply', () => {
  it('splits CMD, SEQ, STATUS and the 29-byte payload', () => {
    const frame = new Uint8Array(MSG_LEN);
    frame[0] = 0x82;
    frame[1] = 0x05;
    frame[2] = Status.Ok;
    frame[3] = 0xde;
    frame[4] = 0xad;
    const reply = decodeReply(frame);
    expect(reply.cmd).toBe(0x82);
    expect(reply.seq).toBe(0x05);
    expect(reply.status).toBe(Status.Ok);
    expect(reply.payload.length).toBe(MSG_LEN - 3);
    expect([reply.payload[0], reply.payload[1]]).toEqual([0xde, 0xad]);
  });

  it('decodes every Status value verbatim', () => {
    for (const status of [
      Status.Ok,
      Status.BadCmd,
      Status.BadArg,
      Status.Busy,
      Status.Unsupported,
    ]) {
      const frame = new Uint8Array(MSG_LEN);
      frame[2] = status;
      expect(decodeReply(frame).status).toBe(status);
    }
  });

  it('throws on a frame that is not exactly 32 bytes', () => {
    expect(() => decodeReply(new Uint8Array(31))).toThrow(RangeError);
    expect(() => decodeReply(new Uint8Array(33))).toThrow(RangeError);
  });
});

describe('round-trip against the firmware fixture', () => {
  it('reply CMD is request CMD with REPLY_FLAG set and SEQ echoed', () => {
    const request = encodeRequest(0x02, 0x7a);
    const reply = decodeReply(fakeFirmwareHandle(request));
    expect(reply.cmd).toBe(0x02 | REPLY_FLAG);
    expect(reply.seq).toBe(0x7a);
    expect(reply.status).toBe(Status.Ok);
  });

  it('an unknown group answers UNSUPPORTED', () => {
    // 0xBx is group 0xB — a nibble no firmware group uses (it is absent from
    // CAPABILITIES = 0xA7FF; groups 0x9 = TEXT and 0xA = UNICODE are both claimed),
    // so handle() falls through to UNSUPPORTED.
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(0xb0, 0x01)));
    expect(reply.status).toBe(Status.Unsupported);
  });

  it('an unknown INFO operation answers BAD_CMD', () => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(0x0f, 0x01)));
    expect(reply.status).toBe(Status.BadCmd);
  });

  it('an unknown operation in a registry-routed group answers BAD_CMD', () => {
    // MACRO (0x5x) and BEHAVIOR (0x7x) are owned by registry features and routed
    // through the firmware's `run_on_kcp`; an op the features do not claim is still
    // a known group, so it answers BAD_CMD — not the UNSUPPORTED an unknown *group*
    // gets. 0x5f is a free MACRO opcode (only 0x50..0x56 are assigned); the BEHAVIOR
    // range is fully assigned today but shares this contract via the same catch-all.
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(0x5f, 0x01)));
    expect(reply.status).toBe(Status.BadCmd);
  });
});

describe('SeqCounter', () => {
  it('returns sequential tags', () => {
    const seq = new SeqCounter();
    expect([seq.next(), seq.next(), seq.next()]).toEqual([0, 1, 2]);
  });

  it('wraps at 256', () => {
    const seq = new SeqCounter(0xfe);
    expect([seq.next(), seq.next(), seq.next()]).toEqual([0xfe, 0xff, 0x00]);
  });
});
