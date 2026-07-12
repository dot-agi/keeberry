// SPDX-License-Identifier: GPL-2.0-or-later
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import { describe, expect, it } from 'vitest';
import { KcpClient } from '../kcp/client';
import { Cmd } from '../kcp/protocol';
import { UnicodeMode } from '../kcp/unicode';
import { createFakeDevice, fakeFirmwareHandle, type FakeDevice } from '../kcp/firmware-fixture';
import type {
  ReportListener,
  Transport,
  TransportDevice,
  Unsubscribe,
} from '../kcp/transport-iface';
import { evalShowIf, readControlValue, writeControlValue } from '../featureDescriptors/runtime';
import type { Control, FeatureDescriptor, OpRunner } from '../featureDescriptors/types';
import { DescriptorPanel } from './DescriptorPanel';

/**
 * A fixture-backed {@link TransportDevice} (the same transport seam client.test.ts uses) so
 * the descriptor runtime round-trips the real wire format through a real {@link KcpClient} —
 * exercising the generic `runOp` path the panel depends on — with no WebHID and no hardware.
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

class FakeTransport implements Transport {
  constructor(private readonly device: TransportDevice) {}

  isSupported(): boolean {
    return true;
  }

  async requestDevice(): Promise<TransportDevice | null> {
    return this.device;
  }
}

async function connectFakeClient(device: FakeDevice = createFakeDevice()): Promise<KcpClient> {
  const client = await KcpClient.request(new FakeTransport(new FakeTransportDevice(device)));
  if (!client) throw new Error('expected the fake transport to yield a client');
  return client;
}

/** A descriptor with one of every control kind, plus a control hidden by `showIf`. */
const sampleDescriptor: FeatureDescriptor = {
  fid: 0x2001,
  title: 'Sample feature',
  controls: [
    {
      kind: 'toggle',
      label: 'Power',
      token: 'power',
      get: { cmd: Cmd.RgbGetState, at: 5 },
      set: { cmd: Cmd.RgbSetEnabled },
    },
    {
      kind: 'enum',
      label: 'Effect',
      options: [
        { label: 'Solid', value: 0 },
        { label: 'Breathing', value: 1 },
      ],
      get: { cmd: Cmd.RgbGetState, at: 0 },
      set: { cmd: Cmd.RgbSetMode },
    },
    {
      kind: 'slider',
      label: 'Brightness',
      min: 0,
      max: 255,
      get: { cmd: Cmd.RgbGetState, at: 4 },
      set: { cmd: Cmd.RgbSetBrightness },
    },
    {
      kind: 'number',
      label: 'Speed',
      min: 0,
      max: 255,
      get: { cmd: Cmd.RgbGetState, at: 8 },
      set: { cmd: Cmd.RgbSetSpeed },
    },
    {
      kind: 'color',
      label: 'Color',
      get: { cmd: Cmd.RgbGetState, at: 1 },
      set: { cmd: Cmd.RgbSetHsv },
    },
    {
      kind: 'toggle',
      label: 'Hidden Extra',
      showIf: 'power == 9',
      get: { cmd: Cmd.RgbGetState, at: 5 },
      set: { cmd: Cmd.RgbSetEnabled },
    },
  ],
};

describe('DescriptorPanel — renders each control kind by kind', () => {
  const stub: OpRunner = { runOp: async () => new Uint8Array(32) };
  const markup = renderToStaticMarkup(
    createElement(DescriptorPanel, { descriptor: sampleDescriptor, client: stub }),
  );

  it('draws the matching widget for every kind', () => {
    expect(markup).toContain('Sample feature'); // the descriptor title
    expect(markup).toContain('Power'); // toggle label
    expect(markup).toContain('>Off<'); // toggle segmented control
    expect(markup).toContain('<select'); // enum
    expect(markup).toContain('type="range"'); // slider + colour channels
    expect(markup).toContain('type="number"'); // number
    expect(markup).toContain('Color'); // colour picker label
    expect(markup).toContain('background-color'); // colour preview swatch
  });

  it('hides a control whose showIf is false (the LSP ignore-unknown rule)', () => {
    // `power` seeds to 0 (toggle default) before the get resolves, so `power == 9` is false.
    expect(markup).not.toContain('Hidden Extra');
  });
});

describe('DescriptorPanel runtime — get/set round-trip via the firmware fixture', () => {
  it('round-trips a toggle (autocorrect TEXT ops, value in reply byte 0)', async () => {
    const client = await connectFakeClient();
    const toggle: Control = {
      kind: 'toggle',
      label: 'Autocorrect',
      get: { cmd: Cmd.TextAutocorrectInfo },
      set: { cmd: Cmd.TextAutocorrectSet },
    };

    await writeControlValue(client, toggle, 0);
    expect(await readControlValue(client, toggle)).toBe(0);
    await writeControlValue(client, toggle, 1);
    expect(await readControlValue(client, toggle)).toBe(1);
  });

  it('round-trips an enum (unicode OS input mode)', async () => {
    const client = await connectFakeClient();
    const mode: Control = {
      kind: 'enum',
      label: 'OS input mode',
      options: [
        { label: 'Linux', value: UnicodeMode.Linux },
        { label: 'Windows', value: UnicodeMode.Windows },
      ],
      get: { cmd: Cmd.UnicodeGet },
      set: { cmd: Cmd.UnicodeSetMode },
    };

    await writeControlValue(client, mode, UnicodeMode.Windows);
    expect(await readControlValue(client, mode)).toBe(UnicodeMode.Windows);
    await writeControlValue(client, mode, UnicodeMode.Linux);
    expect(await readControlValue(client, mode)).toBe(UnicodeMode.Linux);
  });

  it('round-trips a slider through a struct-offset get (RGB brightness @4)', async () => {
    const client = await connectFakeClient();
    const brightness: Control = {
      kind: 'slider',
      label: 'Brightness',
      min: 0,
      max: 255,
      get: { cmd: Cmd.RgbGetState, at: 4 },
      set: { cmd: Cmd.RgbSetBrightness },
    };

    await writeControlValue(client, brightness, 200);
    expect(await readControlValue(client, brightness)).toBe(200);
  });

  it('round-trips a colour as three HSV bytes (RGB GET_STATE @1 / SET_HSV)', async () => {
    const client = await connectFakeClient();
    const color: Control = {
      kind: 'color',
      label: 'Color',
      get: { cmd: Cmd.RgbGetState, at: 1 },
      set: { cmd: Cmd.RgbSetHsv },
    };

    await writeControlValue(client, color, [10, 20, 30]);
    expect(await readControlValue(client, color)).toEqual([10, 20, 30]);
  });

  it('appends the new value after a set op fixed prefix args', async () => {
    const sent: Array<{ cmd: number; args?: number[] }> = [];
    const runner: OpRunner = {
      runOp: async (cmd, args) => {
        sent.push({ cmd, args });
        return new Uint8Array();
      },
    };
    const control: Control = {
      kind: 'slider',
      label: 'X',
      min: 0,
      max: 255,
      get: { cmd: 0x10 },
      set: { cmd: 0x11, args: [2, 3] },
    };

    await writeControlValue(runner, control, 7);
    expect(sent).toEqual([{ cmd: 0x11, args: [2, 3, 7] }]);
  });
});

describe('evalShowIf — the showIf mini-language', () => {
  it('evaluates comparisons, boolean connectives, parens and bare tokens', () => {
    expect(evalShowIf('a == 1', { a: 1 })).toBe(true);
    expect(evalShowIf('a == 1', { a: 0 })).toBe(false);
    expect(evalShowIf('a != 2', { a: 1 })).toBe(true);
    expect(evalShowIf('a < 3 && b > 1', { a: 2, b: 2 })).toBe(true);
    expect(evalShowIf('a < 3 && b > 1', { a: 2, b: 0 })).toBe(false);
    expect(evalShowIf('a == 1 || b == 1', { a: 0, b: 1 })).toBe(true);
    expect(evalShowIf('(a == 1 || b == 1) && c == 1', { a: 1, b: 0, c: 1 })).toBe(true);
    expect(evalShowIf('flag', { flag: 1 })).toBe(true);
    expect(evalShowIf('flag', { flag: 0 })).toBe(false);
  });

  it('throws on a malformed expression or an unknown token (the panel then fails open)', () => {
    expect(() => evalShowIf('a ==', { a: 1 })).toThrow();
    expect(() => evalShowIf('a @ b', { a: 1 })).toThrow();
    expect(() => evalShowIf('(a == 1', { a: 1 })).toThrow();
    expect(() => evalShowIf('missing == 0', {})).toThrow(); // an unresolved token fails open, not 0
  });
});
