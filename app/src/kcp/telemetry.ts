// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * TELEMETRY group (0x2x) parser. Decodes the live snapshot the firmware packs
 * in `kcp.rs`'s `pack_telemetry`, every multi-byte field little-endian. Layout,
 * with payload-relative byte offsets (TELEMETRY_LEN = 23):
 *
 * | bytes    | field                                              |
 * |----------|----------------------------------------------------|
 * | `0..4`   | uptime since boot, ms (u32)                        |
 * | `4..8`   | total matrix scans (u32)                           |
 * | `8..12`  | total HID reports written (u32)                    |
 * | `12..14` | active-layer bitmask (u16, bit n = layer n)        |
 * | `14..16` | scan rate, Hz (u16; SCAN_RATE_HZ = 1000)           |
 * | `16..20` | last-iteration processing time, µs (u32)           |
 * | `20`     | battery percent (0xFF = unavailable)               |
 * | `21`     | link RSSI (0xFF = unavailable)                     |
 * | `22`     | connection / transport (wireless::Devs code)       |
 */
import { readU16LE, readU32LE } from './bytes';

/** Sentinel byte for a telemetry field with no value on the current link. */
export const TELEMETRY_UNAVAILABLE = 0xff;

/** A decoded telemetry snapshot. */
export interface Telemetry {
  /** Uptime since boot, milliseconds (wraps after ~49.7 days). */
  uptimeMs: number;
  /** Total matrix scans since boot. */
  scanCount: number;
  /** Total HID keyboard reports written since boot. */
  reportCount: number;
  /** Active-layer bitmask (bit `n` set means layer `n` is active). */
  activeLayers: number;
  /** Nominal matrix scan rate, Hz (the firmware's fixed SCAN_RATE_HZ). */
  scanRateHz: number;
  /** Processing time of the last keyboard-loop iteration, microseconds. */
  lastProcUs: number;
  /** Battery percent, or `null` when unavailable (`0xFF`). */
  battery: number | null;
  /** Link RSSI, or `null` when unavailable (`0xFF`). */
  rssi: number | null;
  /** Active transport / connection code (`wireless::Devs`). */
  connection: number;
}

/** Decode the active-layer bitmask into the list of active layer indices. */
export function activeLayerList(mask: number): number[] {
  const layers: number[] = [];
  for (let bit = 0; bit < 16; bit += 1) {
    if ((mask & (1 << bit)) !== 0) layers.push(bit);
  }
  return layers;
}

/** Parse a GET_TELEMETRY reply payload into a {@link Telemetry} snapshot. */
export function parseTelemetry(payload: Uint8Array): Telemetry {
  const battery = payload[20];
  const rssi = payload[21];
  return {
    uptimeMs: readU32LE(payload, 0),
    scanCount: readU32LE(payload, 4),
    reportCount: readU32LE(payload, 8),
    activeLayers: readU16LE(payload, 12),
    scanRateHz: readU16LE(payload, 14),
    lastProcUs: readU32LE(payload, 16),
    battery: battery === TELEMETRY_UNAVAILABLE ? null : battery,
    rssi: rssi === TELEMETRY_UNAVAILABLE ? null : rssi,
    connection: payload[22],
  };
}
