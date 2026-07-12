// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * kcp frame codec — pure, allocation-light encode/decode of the fixed 32-byte
 * request and reply frames. No WebHID here, so this module is trivially
 * unit-testable and is the correctness gate for the wire format.
 */
import {
  CMD_IDX,
  MSG_LEN,
  REPLY_PAYLOAD_IDX,
  REQ_PAYLOAD_IDX,
  REQ_PAYLOAD_LEN,
  SEQ_IDX,
  STATUS_IDX,
  Status,
} from './protocol';

/** A decoded reply frame. `payload` is the 29-byte reply payload (`frame[3..32]`). */
export interface DecodedReply {
  /** CMD byte as received (reply has bit 7 set). */
  cmd: number;
  /** Echoed sequence tag. */
  seq: number;
  /** STATUS byte. */
  status: Status;
  /** Reply payload, `frame[3..32]` (29 bytes). */
  payload: Uint8Array;
  /** The full 32-byte frame, for debugging / hex dumps. */
  raw: Uint8Array;
}

/**
 * Encode a request into a fixed 32-byte frame:
 * `[0]=cmd [1]=seq [2..32]=payload` (payload zero-padded to 30 bytes).
 *
 * @throws RangeError if the payload exceeds the 30-byte request region.
 */
export function encodeRequest(
  cmd: number,
  seq: number,
  payload?: ArrayLike<number>,
): Uint8Array<ArrayBuffer> {
  const frame = new Uint8Array(MSG_LEN);
  frame[CMD_IDX] = cmd & 0xff;
  frame[SEQ_IDX] = seq & 0xff;
  if (payload && payload.length > 0) {
    if (payload.length > REQ_PAYLOAD_LEN) {
      throw new RangeError(
        `kcp request payload of ${payload.length} bytes exceeds ${REQ_PAYLOAD_LEN}`,
      );
    }
    frame.set(
      Array.from(payload, (b) => b & 0xff),
      REQ_PAYLOAD_IDX,
    );
  }
  return frame;
}

/**
 * Decode a 32-byte reply frame: `[0]=cmd [1]=seq [2]=status [3..32]=payload`.
 *
 * @throws RangeError if the frame is not exactly 32 bytes.
 */
export function decodeReply(frame: Uint8Array): DecodedReply {
  if (frame.length !== MSG_LEN) {
    throw new RangeError(`kcp reply frame must be ${MSG_LEN} bytes, got ${frame.length}`);
  }
  return {
    cmd: frame[CMD_IDX],
    seq: frame[SEQ_IDX],
    status: frame[STATUS_IDX] as Status,
    payload: frame.slice(REPLY_PAYLOAD_IDX),
    raw: frame,
  };
}

/**
 * Monotonic 8-bit sequence tag generator. SEQ is opaque to the firmware (it is
 * echoed verbatim), so a simple wrapping counter is all the host needs to pair
 * replies to requests.
 */
export class SeqCounter {
  private value: number;

  constructor(start = 0) {
    this.value = start & 0xff;
  }

  /** Return the next sequence tag and advance, wrapping at 256. */
  next(): number {
    const seq = this.value;
    this.value = (this.value + 1) & 0xff;
    return seq;
  }
}
