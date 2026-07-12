// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * INFO group (0x0x) typed parsers. Every layout here mirrors the packing in
 * `firmware/src/kcp.rs` byte-for-byte; the offsets are payload-relative
 * (i.e. into `reply[3..32]`, the 29-byte reply payload).
 */
import { readU16LE, readU32LE } from './bytes';
import { Group } from './protocol';

// === Protocol version (CMD_GET_VERSION, 0x00) ==============================
// `kcp.rs`: reply payload = PROTOCOL_VERSION = [major, minor].

export interface ProtocolVersion {
  major: number;
  minor: number;
}

/** Parse the `[major, minor]` protocol version from a GET_VERSION reply payload. */
export function parseProtocolVersion(payload: Uint8Array): ProtocolVersion {
  return { major: payload[0], minor: payload[1] };
}

/** Format a protocol version as `major.minor`. */
export function formatProtocolVersion(v: ProtocolVersion): string {
  return `${v.major}.${v.minor}`;
}

// === Capabilities (CMD_GET_CAPABILITIES, 0x01) =============================
// `kcp.rs`: reply payload = CAPABILITIES as a little-endian u32. Bit `g` is set
// when command group `g` is implemented (bit 0 = INFO, … bit 0xD = FEATURES,
// bit 0xF = SYSTEM). The group bits are sparse (0xB, 0xC, 0xE are unused), so
// {@link GROUP_DEFS} below is the authoritative bit-to-group map.

/** Stable identifiers for the command groups, in bit order. */
export type GroupName =
  | 'info'
  | 'keymap'
  | 'telemetry'
  | 'hidKro'
  | 'config'
  | 'macro'
  | 'rgb'
  | 'behavior'
  | 'wireless'
  | 'text'
  | 'unicode'
  | 'features'
  | 'system';

interface GroupDef {
  name: GroupName;
  /** Bit index in the capabilities mask; equals the CMD group nibble. */
  bit: number;
  /** Human-readable label for the UI. */
  label: string;
}

/** The command groups and their capability-bit indices (mirror of `mod group`). */
export const GROUP_DEFS: readonly GroupDef[] = [
  { name: 'info', bit: Group.Info, label: 'Info' },
  { name: 'keymap', bit: Group.Keymap, label: 'Keymap' },
  { name: 'telemetry', bit: Group.Telemetry, label: 'Telemetry' },
  { name: 'hidKro', bit: Group.HidKro, label: 'HID / KRO' },
  { name: 'config', bit: Group.Config, label: 'Config' },
  { name: 'macro', bit: Group.Macro, label: 'Macro' },
  { name: 'rgb', bit: Group.Rgb, label: 'RGB' },
  { name: 'behavior', bit: Group.Behavior, label: 'Behavior' },
  { name: 'wireless', bit: Group.Wireless, label: 'Wireless' },
  { name: 'text', bit: Group.Text, label: 'Text' },
  { name: 'unicode', bit: Group.Unicode, label: 'Unicode' },
  { name: 'features', bit: Group.Features, label: 'Features' },
  { name: 'system', bit: Group.System, label: 'System' },
] as const;

export type GroupFlags = Record<GroupName, boolean>;

export interface Capabilities {
  /** The raw little-endian u32 bitmask as decoded from the payload. */
  raw: number;
  /** Per-group present/absent flags. */
  groups: GroupFlags;
  /** The groups that are present, in bit order (handy for the UI). */
  present: GroupName[];
  /**
   * Bit indices that are set in `raw` but map to no known group — a forward
   * compatibility escape hatch so a newer firmware never breaks the GUI.
   */
  unknownBits: number[];
}

/** Decode the capabilities bitmask from a GET_CAPABILITIES reply payload. */
export function parseCapabilities(payload: Uint8Array): Capabilities {
  const raw = readU32LE(payload);

  const groups = {} as GroupFlags;
  const present: GroupName[] = [];
  let knownMask = 0;
  for (const def of GROUP_DEFS) {
    knownMask |= 1 << def.bit;
    const isSet = (raw & (1 << def.bit)) !== 0;
    groups[def.name] = isSet;
    if (isSet) {
      present.push(def.name);
    }
  }

  const unknownBits: number[] = [];
  const leftover = raw & ~knownMask;
  for (let bit = 0; bit < 32; bit += 1) {
    if ((leftover & (1 << bit)) !== 0) {
      unknownBits.push(bit);
    }
  }

  return { raw, groups, present, unknownBits };
}

/** Look up a group's display label by identifier. */
export function groupLabel(name: GroupName): string {
  return GROUP_DEFS.find((g) => g.name === name)?.label ?? name;
}

// === Device info (CMD_GET_DEVICE_INFO, 0x02) ===============================
// `kcp.rs` pack_device_info, payload-relative offsets (DEVICE_INFO_LEN = 17):
//   0..3   firmware version [major, minor, patch]
//   3..11  chip id, 8 ASCII bytes ("WB32FQ95")
//   11     matrix rows  (NUM_ROWS)
//   12     matrix cols  (NUM_COLS)
//   13     layer count  (LAYERS)
//   14     transport / connection (0 = USB)
//   15..17 config schema version (config::SCHEMA_VERSION, u16 LE)

export interface FirmwareVersion {
  major: number;
  minor: number;
  patch: number;
}

export interface Connection {
  /** The raw transport byte. */
  code: number;
  /** Human-readable label. */
  label: string;
}

export interface DeviceInfo {
  firmwareVersion: FirmwareVersion;
  /** `major.minor.patch`. */
  firmwareVersionString: string;
  /** Chip identifier, e.g. `WB32FQ95`. */
  chip: string;
  /** Matrix rows (`NUM_ROWS`). */
  rows: number;
  /** Matrix columns (`NUM_COLS`). */
  cols: number;
  /** Keymap layer count (`LAYERS`). */
  layers: number;
  /** Active transport the request arrived on. */
  connection: Connection;
  /**
   * On-flash config schema version (`config::SCHEMA_VERSION`). The persist-
   * across-flash compatibility key: a config blob is restorable into this
   * firmware only when its schema version matches this one exactly (the firmware
   * does an exact-match check, never a migration).
   */
  schemaVersion: number;
}

/**
 * Transport byte -> label. Values are the firmware `wireless::Devs` codes
 * (`Usb = 0`, `Bt1 = 1`, `Bt2 = 2`, `Bt3 = 3`, `G2_4 = 6`); GET_DEVICE_INFO
 * reports `0` (USB) today, the rest arrive with the WIRELESS group.
 */
export const CONNECTION_LABELS: Record<number, string> = {
  0: 'USB',
  1: 'Bluetooth 1',
  2: 'Bluetooth 2',
  3: 'Bluetooth 3',
  6: '2.4 GHz',
};

/** Human-readable label for a transport byte (handles unknown codes). */
export function connectionLabel(code: number): string {
  return CONNECTION_LABELS[code] ?? 'Unknown';
}

/** Decode an 8-byte ASCII field, trimming trailing NULs. */
function readAscii(payload: Uint8Array, start: number, length: number): string {
  let end = start + length;
  while (end > start && payload[end - 1] === 0) {
    end -= 1;
  }
  let out = '';
  for (let i = start; i < end; i += 1) {
    out += String.fromCharCode(payload[i]);
  }
  return out;
}

/** Decode the static device descriptor from a GET_DEVICE_INFO reply payload. */
export function parseDeviceInfo(payload: Uint8Array): DeviceInfo {
  const firmwareVersion: FirmwareVersion = {
    major: payload[0],
    minor: payload[1],
    patch: payload[2],
  };
  const code = payload[14];
  return {
    firmwareVersion,
    firmwareVersionString: `${firmwareVersion.major}.${firmwareVersion.minor}.${firmwareVersion.patch}`,
    chip: readAscii(payload, 3, 8),
    rows: payload[11],
    cols: payload[12],
    layers: payload[13],
    connection: { code, label: connectionLabel(code) },
    schemaVersion: readU16LE(payload, 15),
  };
}
