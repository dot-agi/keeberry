// SPDX-License-Identifier: GPL-2.0-or-later
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { KcpConnection, KcpTimeoutError } from './transport';
import type { ReportListener, TransportDevice, Unsubscribe } from './transport-iface';
import { Cmd, REPLY_FLAG, Status } from './protocol';
import { parseDeviceInfo, parseProtocolVersion } from './info';
import { fakeFirmwareHandle } from './firmware-fixture';

/**
 * Minimal {@link TransportDevice} that answers with the firmware fixture. It can
 * respond immediately or queue replies for out-of-order delivery, and can be told
 * to fail a write. Exercising {@link KcpConnection} through this fake keeps the
 * SEQ/CMD pairing under test without any WebHID dependency.
 */
class FakeTransportDevice implements TransportDevice {
  readonly name = 'keeberry (fake)';
  sendShouldReject = false;

  readonly sent: Uint8Array[] = [];
  private readonly queued: Uint8Array[] = [];
  private readonly listeners = new Set<ReportListener>();

  constructor(private readonly autoRespond = true) {}

  async open(): Promise<void> {}

  async write(report: Uint8Array): Promise<void> {
    if (this.sendShouldReject) {
      throw new Error('device write failed');
    }
    const request = report.slice();
    this.sent.push(request);
    const reply = fakeFirmwareHandle(request);
    if (this.autoRespond) {
      this.deliver(reply);
    } else {
      this.queued.push(reply);
    }
  }

  subscribe(listener: ReportListener): Unsubscribe {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  }

  onDisconnect(): Unsubscribe {
    return () => {};
  }

  async close(): Promise<void> {}

  /** Dispatch an arbitrary 32-byte frame as an inbound report. */
  deliver(frame: Uint8Array): void {
    for (const listener of this.listeners) {
      listener(frame);
    }
  }

  /** Flush queued replies last-in-first-out, to model out-of-order delivery. */
  flushReverse(): void {
    while (this.queued.length > 0) {
      this.deliver(this.queued.pop()!);
    }
  }

  /** The most recent request's SEQ byte. */
  lastSeq(): number {
    return this.sent[this.sent.length - 1][1];
  }
}

/**
 * Flush the microtask queue so the serialized transact chain writes its request
 * before the test inspects `device.sent` / `lastSeq()`. Fake timers do not affect
 * microtasks, so awaiting resolved promises drains the enqueue chain.
 */
const flushMicrotasks = async (): Promise<void> => {
  for (let tick = 0; tick < 5; tick += 1) {
    await Promise.resolve();
  }
};

beforeEach(() => {
  vi.useFakeTimers();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('KcpConnection.transact', () => {
  it('resolves with the SEQ- and CMD-matched reply', async () => {
    const device = new FakeTransportDevice();
    const connection = new KcpConnection(device);

    const reply = await connection.transact(Cmd.GetVersion);
    expect(reply.cmd).toBe(Cmd.GetVersion | REPLY_FLAG);
    expect(reply.seq).toBe(device.lastSeq());
    expect(reply.status).toBe(Status.Ok);
    expect(parseProtocolVersion(reply.payload)).toEqual({ major: 0, minor: 2 });
  });

  it('feeds the transport reply straight into the INFO parsers', async () => {
    const device = new FakeTransportDevice();
    const connection = new KcpConnection(device);

    const reply = await connection.transact(Cmd.GetDeviceInfo);
    expect(parseDeviceInfo(reply.payload)).toMatchObject({
      chip: 'WB32FQ95',
      rows: 6,
      cols: 15,
      layers: 16,
    });
  });

  it('serializes concurrent transacts, holding each write until the prior reply', async () => {
    const device = new FakeTransportDevice(false);
    const connection = new KcpConnection(device);

    const versionPromise = connection.transact(Cmd.GetVersion);
    const infoPromise = connection.transact(Cmd.GetDeviceInfo);

    // Only the first request reaches the wire; the second waits behind it.
    await flushMicrotasks();
    expect(device.sent).toHaveLength(1);

    // Releasing the first reply unblocks the second request's write.
    device.flushReverse();
    const version = await versionPromise;
    await flushMicrotasks();
    expect(device.sent).toHaveLength(2);

    device.flushReverse();
    const info = await infoPromise;

    expect(version.cmd).toBe(Cmd.GetVersion | REPLY_FLAG);
    expect(info.cmd).toBe(Cmd.GetDeviceInfo | REPLY_FLAG);
    expect(parseProtocolVersion(version.payload)).toEqual({ major: 0, minor: 2 });
    expect(parseDeviceInfo(info.payload).chip).toBe('WB32FQ95');
    expect(device.sent.map((frame) => frame[0])).toEqual([Cmd.GetVersion, Cmd.GetDeviceInfo]);
  });

  it('ignores a reply whose CMD does not match the request and times out', async () => {
    const device = new FakeTransportDevice(false);
    const connection = new KcpConnection(device);

    const pending = connection.transact(Cmd.GetCapabilities, undefined, { timeoutMs: 1000 });
    const rejection = expect(pending).rejects.toBeInstanceOf(KcpTimeoutError);
    // The serialized write is deferred to a microtask; let it reach the wire so
    // its SEQ is known before we forge a reply.
    await flushMicrotasks();

    // Right SEQ, wrong (reply-flagged) CMD -> must not resolve the request.
    const stray = new Uint8Array(32);
    stray[0] = (Cmd.GetDeviceInfo | REPLY_FLAG) & 0xff;
    stray[1] = device.lastSeq();
    stray[2] = Status.Ok;
    device.deliver(stray);

    await vi.advanceTimersByTimeAsync(1000);
    await rejection;
  });

  it('rejects with KcpTimeoutError when no reply arrives', async () => {
    const device = new FakeTransportDevice(false);
    const connection = new KcpConnection(device);

    const pending = connection.transact(Cmd.GetVersion, undefined, { timeoutMs: 500 });
    const rejection = expect(pending).rejects.toBeInstanceOf(KcpTimeoutError);
    await vi.advanceTimersByTimeAsync(500);
    await rejection;
  });

  it('rejects when the OUT report cannot be written', async () => {
    const device = new FakeTransportDevice();
    device.sendShouldReject = true;
    const connection = new KcpConnection(device);

    await expect(connection.transact(Cmd.GetVersion)).rejects.toThrow('device write failed');
  });
});

describe('KcpConnection.send (fire-and-forget, no reply awaited)', () => {
  it('writes the request frame and resolves without awaiting a reply', async () => {
    const device = new FakeTransportDevice(false); // never delivers a reply
    const connection = new KcpConnection(device);

    await connection.send(Cmd.SystemEnterDfu);
    expect(device.sent).toHaveLength(1);
    expect(device.sent[0][0]).toBe(Cmd.SystemEnterDfu);
  });

  it('propagates a write failure to the caller', async () => {
    const device = new FakeTransportDevice();
    device.sendShouldReject = true;
    const connection = new KcpConnection(device);

    await expect(connection.send(Cmd.SystemReboot)).rejects.toThrow('device write failed');
  });
});

describe('KcpConnection.close', () => {
  it('detaches the input listener and rejects in-flight requests', async () => {
    const device = new FakeTransportDevice(false);
    const connection = new KcpConnection(device);

    const pending = connection.transact(Cmd.GetVersion);
    const rejection = expect(pending).rejects.toThrow('kcp connection closed');
    connection.close();
    await rejection;

    // After close, a late reply is dropped instead of throwing.
    expect(() => device.flushReverse()).not.toThrow();
  });
});
