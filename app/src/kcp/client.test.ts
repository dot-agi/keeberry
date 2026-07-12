// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { KcpClient } from './client';
import type { ReportListener, Transport, TransportDevice, Unsubscribe } from './transport-iface';
import { FEATURE_DEFS, createFakeDevice, fakeFirmwareHandle, type FakeDevice } from './firmware-fixture';

/**
 * A {@link TransportDevice} that answers from the stateful firmware fixture,
 * delivering each reply synchronously. It drives a real {@link KcpClient} — whose
 * constructor is private, reachable only through {@link KcpClient.request} — so the
 * suite exercises the real wire path (and so the real unsaved-state logic) over the
 * transport seam, with no WebHID stub.
 */
class FakeTransportDevice implements TransportDevice {
  readonly name = 'keeberry (fake)';
  private listener: ReportListener | null = null;

  constructor(private readonly device: FakeDevice) {}

  async open(): Promise<void> {}

  async write(report: Uint8Array): Promise<void> {
    this.listener?.(fakeFirmwareHandle(report, this.device));
  }

  subscribe(listener: ReportListener): Unsubscribe {
    this.listener = listener;
    return () => {
      this.listener = null;
    };
  }

  onDisconnect(): Unsubscribe {
    return () => {};
  }

  async close(): Promise<void> {}
}

/** A {@link Transport} that hands back one fixture-backed device. */
class FakeTransport implements Transport {
  constructor(private readonly device: TransportDevice) {}

  isSupported(): boolean {
    return true;
  }

  async requestDevice(): Promise<TransportDevice | null> {
    return this.device;
  }
}

/** Connect a real client over a fixture-backed fake transport. */
async function connectFakeClient(device: FakeDevice = createFakeDevice()): Promise<KcpClient> {
  const transport = new FakeTransport(new FakeTransportDevice(device));
  const client = await KcpClient.request(transport);
  if (!client) {
    throw new Error('expected the fake transport to yield a client');
  }
  return client;
}

describe('KcpClient unsaved-changes tracking', () => {
  it('starts clean, a persisted setter marks it dirty, and CONFIG.SAVE clears it', async () => {
    const client = await connectFakeClient();
    expect(client.hasUnsavedChanges).toBe(false);

    await client.setKeycode(0, 0, 0, 0x04);
    expect(client.hasUnsavedChanges).toBe(true);

    // SAVE is the only op that persists everything, so it is the only clear.
    await client.configSave();
    expect(client.hasUnsavedChanges).toBe(false);
  });

  it('CONFIG.LOAD_DEFAULTS marks it dirty — defaults are RAM-only until a SAVE', async () => {
    const client = await connectFakeClient();
    expect(client.hasUnsavedChanges).toBe(false);

    // Loading defaults overwrites live RAM without persisting, so it is itself an
    // unsaved change; only a following SAVE clears the flag again.
    await client.configLoadDefaults();
    expect(client.hasUnsavedChanges).toBe(true);

    await client.configSave();
    expect(client.hasUnsavedChanges).toBe(false);
  });

  it('a live-but-not-persisted op leaves the flag untouched', async () => {
    const client = await connectFakeClient();

    // WIRELESS pairing changes nothing the flash blob holds.
    await client.wirelessPair();
    expect(client.hasUnsavedChanges).toBe(false);

    // The UNICODE codepoint map is RAM-only — re-uploaded by the host on connect,
    // never written to flash — so a SET_MAP is not an unsaved change either.
    await client.unicodeSetMap(0, 0x1f600);
    expect(client.hasUnsavedChanges).toBe(false);
  });

  it('an autocorrect SET is a persisted change — live, but unsaved until a SAVE', async () => {
    const client = await connectFakeClient();
    expect(client.hasUnsavedChanges).toBe(false);

    await client.setAutocorrect(false);
    expect(client.hasUnsavedChanges).toBe(true);

    await client.configSave();
    expect(client.hasUnsavedChanges).toBe(false);
  });
});

describe('KcpClient TEXT group (autocorrect)', () => {
  it('reads the boot state and round-trips a disable through the wire', async () => {
    const client = await connectFakeClient();

    const initial = await client.getAutocorrect();
    expect(initial.enabled).toBe(true);
    expect(initial.entryCount).toBeGreaterThan(0);

    await client.setAutocorrect(false);
    expect((await client.getAutocorrect()).enabled).toBe(false);
  });
});

describe('KcpClient FEATURES group', () => {
  it('lists every registered feature (paging the enumeration), all on at boot', async () => {
    const client = await connectFakeClient();
    const features = await client.listFeatures();
    expect(features.map((f) => f.name)).toEqual(FEATURE_DEFS.map((f) => f.name));
    expect(features.every((f) => f.enabled)).toBe(true);
  });

  it('toggles a feature live and observes it on the next list', async () => {
    const client = await connectFakeClient();
    const caps = FEATURE_DEFS.find((f) => f.name === 'Caps Word')!;

    await client.setFeatureEnabled(caps.id, false);
    const after = (await client.listFeatures()).find((f) => f.id === caps.id);
    expect(after?.enabled).toBe(false);
  });

  it('throws when disabling an always-on (structural) feature', async () => {
    const client = await connectFakeClient();
    const timed = FEATURE_DEFS.find((f) => f.name === 'Timed Engine')!;
    await expect(client.setFeatureEnabled(timed.id, false)).rejects.toThrow();
  });

  it('a feature toggle is a persisted change — live, but unsaved until a SAVE', async () => {
    const client = await connectFakeClient();
    const caps = FEATURE_DEFS.find((f) => f.name === 'Caps Word')!;

    await client.setFeatureEnabled(caps.id, false);
    expect(client.hasUnsavedChanges).toBe(true);

    await client.configSave();
    expect(client.hasUnsavedChanges).toBe(false);
  });
});
