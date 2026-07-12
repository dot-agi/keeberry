// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * kcp connection. Operates over a {@link TransportDevice} (WebHID in the browser,
 * Rust in the native build): writes 32-byte OUT reports and resolves each
 * `transact` against the matching 32-byte IN report.
 *
 * A reply pairs with its request when both the echoed SEQ and the CMD match
 * (`reply.cmd === req.cmd | REPLY_FLAG`), which holds for every command group:
 * low groups gain bit 7 in the reply, high groups already have it set and the
 * SEQ disambiguates. The connection is transport-agnostic, and its matching
 * logic is exercised with a fake transport in the unit tests.
 */
import { DecodedReply, decodeReply, encodeRequest, SeqCounter } from './codec';
import { MSG_LEN, REPLY_FLAG } from './protocol';
import type { TransportDevice, Unsubscribe } from './transport-iface';

/** Default time to wait for a reply before rejecting a `transact`. */
export const DEFAULT_TIMEOUT_MS = 2000;

/** Raised when a request is not answered within its timeout. */
export class KcpTimeoutError extends Error {
  constructor(cmd: number, seq: number, timeoutMs: number) {
    super(`kcp transact timed out after ${timeoutMs} ms (cmd=0x${cmd.toString(16)}, seq=${seq})`);
    this.name = 'KcpTimeoutError';
  }
}

export interface TransactOptions {
  /** Override the reply timeout for this request, in milliseconds. */
  timeoutMs?: number;
}

interface Pending {
  cmd: number;
  resolve: (reply: DecodedReply) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
}

/** Copy an inbound report's bytes into a fixed 32-byte frame (zero-padded). */
function frameFromBytes(data: Uint8Array): Uint8Array {
  const frame = new Uint8Array(MSG_LEN);
  frame.set(data.subarray(0, MSG_LEN));
  return frame;
}

/**
 * A live kcp connection over an open {@link TransportDevice}. Construct it with an
 * already opened device; call {@link transact} to exchange messages and
 * {@link close} to detach the inbound-report subscription.
 */
export class KcpConnection {
  private readonly device: TransportDevice;
  private readonly unsubscribe: Unsubscribe;
  private readonly seq = new SeqCounter();
  private readonly pending = new Map<number, Pending>();
  /**
   * Tail of the serialized transact chain. kcp is stop-and-wait: the firmware
   * answers one command at a time, and the stock 2.4 GHz dongle bridges a single
   * 32-byte report at a time. Firing a burst concurrently — e.g. a panel that
   * `Promise.all`s every SOCD / override / tap-dance / combo slot on mount —
   * overruns the radio bridge, which then drops reports that time out. Chaining
   * each {@link transact} behind the previous one's reply keeps exactly one request
   * outstanding on the wire: reliable over the radio, negligibly slower over USB.
   * {@link send} (device resets) deliberately bypasses this.
   */
  private tail: Promise<unknown> = Promise.resolve();
  /**
   * Set once {@link close} runs. A transact still queued behind the serialized
   * chain when the connection closes must reject rather than write to the detached
   * device, so {@link writeAndAwait} checks this before touching the wire.
   */
  private closed = false;

  constructor(device: TransportDevice) {
    this.device = device;
    this.unsubscribe = device.subscribe(this.handleReport);
  }

  private readonly handleReport = (data: Uint8Array): void => {
    const reply = decodeReply(frameFromBytes(data));
    const waiter = this.pending.get(reply.seq);
    // Pair on SEQ and the reply-flagged CMD; ignore anything unsolicited.
    if (!waiter || reply.cmd !== (waiter.cmd | REPLY_FLAG)) {
      return;
    }
    clearTimeout(waiter.timer);
    this.pending.delete(reply.seq);
    waiter.resolve(reply);
  };

  /**
   * Fire-and-forget: write a request and resolve as soon as it is sent, without
   * registering a reply waiter. The SYSTEM group's reset commands never reply —
   * the device resets before it could — so the host issues them this way and
   * treats the ensuing USB disconnect as the acknowledgement; using
   * {@link transact} would always time out. Rejects only if the OUT report
   * itself cannot be written.
   */
  send(cmd: number, payload?: ArrayLike<number>): Promise<void> {
    const seq = this.seq.next();
    return this.device.write(encodeRequest(cmd, seq, payload));
  }

  /**
   * Send a request and resolve with its reply, serialized behind any earlier
   * transact so exactly one request is outstanding on the wire at a time (see
   * {@link tail}). Rejects with {@link KcpTimeoutError} if no matching reply arrives
   * in time, or with the underlying error if the OUT report cannot be written.
   */
  transact(
    cmd: number,
    payload?: ArrayLike<number>,
    options?: TransactOptions,
  ): Promise<DecodedReply> {
    // `tail` is always a settled, error-swallowed promise, so this request runs
    // regardless of whether the previous one resolved or rejected.
    const run = this.tail.then(() => this.writeAndAwait(cmd, payload, options));
    this.tail = run.then(
      () => undefined,
      () => undefined,
    );
    return run;
  }

  /** Write one request frame and resolve against its SEQ/CMD-matched reply. */
  private writeAndAwait(
    cmd: number,
    payload?: ArrayLike<number>,
    options?: TransactOptions,
  ): Promise<DecodedReply> {
    if (this.closed) {
      return Promise.reject(new Error('kcp connection closed'));
    }
    const timeoutMs = options?.timeoutMs ?? DEFAULT_TIMEOUT_MS;
    const seq = this.seq.next();
    const frame = encodeRequest(cmd, seq, payload);

    return new Promise<DecodedReply>((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(seq);
        reject(new KcpTimeoutError(cmd, seq, timeoutMs));
      }, timeoutMs);

      this.pending.set(seq, { cmd, resolve, reject, timer });

      this.device.write(frame).catch((error: unknown) => {
        const waiter = this.pending.get(seq);
        if (waiter) {
          clearTimeout(waiter.timer);
          this.pending.delete(seq);
        }
        reject(error instanceof Error ? error : new Error(String(error)));
      });
    });
  }

  /** Detach the inbound-report subscription and fail any in-flight requests. */
  close(): void {
    this.closed = true;
    this.unsubscribe();
    for (const [seq, waiter] of this.pending) {
      clearTimeout(waiter.timer);
      waiter.reject(new Error('kcp connection closed'));
      this.pending.delete(seq);
    }
  }
}
