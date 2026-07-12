// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  connectionLabel,
  formatProtocolVersion,
  parseCapabilities,
  parseDeviceInfo,
  parseProtocolVersion,
  type GroupName,
} from './info';
import { Group } from './protocol';
import {
  CAPABILITIES,
  SCHEMA_VERSION,
  packCapabilities,
  packDeviceInfo,
  packProtocolVersion,
} from './firmware-fixture';

describe('parseProtocolVersion', () => {
  it('reads [major, minor] as the firmware packs PROTOCOL_VERSION', () => {
    const version = parseProtocolVersion(packProtocolVersion());
    expect(version).toEqual({ major: 0, minor: 2 });
    expect(formatProtocolVersion(version)).toBe('0.2');
  });
});

describe('parseCapabilities', () => {
  it('decodes the firmware CAPABILITIES mask (0xA7FF)', () => {
    const caps = parseCapabilities(packCapabilities());
    expect(CAPABILITIES).toBe(0xa7ff);
    expect(caps.raw).toBe(0xa7ff);
  });

  it('reports exactly the groups present in the firmware today', () => {
    const caps = parseCapabilities(packCapabilities());
    const expectedPresent: GroupName[] = [
      'info',
      'keymap',
      'telemetry',
      'hidKro',
      'config',
      'macro',
      'rgb',
      'behavior',
      'wireless',
      'text',
      'unicode',
      'features',
      'system',
    ];
    expect(caps.present).toEqual(expectedPresent);
    // 0xA7FF lights every group this app knows, with no leftover bits; bit 9 is the
    // TEXT group (autocorrect), bit 10 the UNICODE group and bit 13 the FEATURES group.
    expect(caps.groups.text).toBe(true);
    expect(caps.raw & (1 << 9)).not.toBe(0);
    expect(caps.groups.unicode).toBe(true);
    expect(caps.raw & (1 << 10)).not.toBe(0);
    expect(caps.groups.features).toBe(true);
    expect(caps.raw & (1 << 13)).not.toBe(0);
    expect(caps.unknownBits).toEqual([]);
  });

  it('maps each group to its bit index (little-endian u32)', () => {
    // System is bit 15 (0x8000); a mask of only that bit must light system.
    const caps = parseCapabilities(packCapabilities(1 << Group.System));
    expect(caps.groups.system).toBe(true);
    expect(caps.present).toEqual(['system']);
  });

  it('surfaces bits with no known group as unknownBits (forward compatible)', () => {
    // Bit 11 maps to no group in this firmware revision (TEXT = bit 9 and UNICODE = bit 10
    // are both claimed now), so it stands in for a future group this client does not know.
    const caps = parseCapabilities(packCapabilities((1 << Group.Info) | (1 << 11)));
    expect(caps.groups.info).toBe(true);
    expect(caps.unknownBits).toEqual([11]);
  });

  it('decodes the full high bit without sign error', () => {
    const caps = parseCapabilities(packCapabilities(0x8000_0001 >>> 0));
    expect(caps.raw).toBe(0x8000_0001);
    expect(caps.groups.info).toBe(true);
    expect(caps.unknownBits).toEqual([31]);
  });
});

describe('parseDeviceInfo', () => {
  it('reads the firmware pack_device_info layout byte-for-byte', () => {
    const info = parseDeviceInfo(packDeviceInfo());
    expect(info.firmwareVersion).toEqual({ major: 0, minor: 1, patch: 0 });
    expect(info.firmwareVersionString).toBe('0.1.0');
    expect(info.chip).toBe('WB32FQ95');
    expect(info.rows).toBe(6);
    expect(info.cols).toBe(15);
    expect(info.layers).toBe(16);
    expect(info.connection).toEqual({ code: 0, label: 'USB' });
    expect(info.schemaVersion).toBe(SCHEMA_VERSION);
    expect(info.schemaVersion).toBe(10);
  });

  it('reads the schema version as a u16 little-endian at offset 15', () => {
    // A two-byte value exercises both payload bytes (0x0102 = 258, LE).
    expect(parseDeviceInfo(packDeviceInfo(0, 0x0102)).schemaVersion).toBe(0x0102);
  });

  it('decodes the wireless transport codes in the connection byte', () => {
    expect(parseDeviceInfo(packDeviceInfo(6)).connection).toEqual({ code: 6, label: '2.4 GHz' });
    expect(parseDeviceInfo(packDeviceInfo(1)).connection.label).toBe('Bluetooth 1');
  });

  it('labels an unrecognised transport code rather than failing', () => {
    expect(parseDeviceInfo(packDeviceInfo(0x42)).connection.label).toBe('Unknown');
  });
});

describe('connectionLabel', () => {
  it('maps the wireless::Devs codes', () => {
    expect(connectionLabel(0)).toBe('USB');
    expect(connectionLabel(1)).toBe('Bluetooth 1');
    expect(connectionLabel(2)).toBe('Bluetooth 2');
    expect(connectionLabel(3)).toBe('Bluetooth 3');
    expect(connectionLabel(6)).toBe('2.4 GHz');
    expect(connectionLabel(9)).toBe('Unknown');
  });
});
