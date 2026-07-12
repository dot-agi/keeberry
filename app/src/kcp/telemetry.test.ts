// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { activeLayerList, parseTelemetry } from './telemetry';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  DEFAULT_TELEMETRY,
  type TelemetrySample,
  fakeFirmwareHandle,
  packTelemetry,
} from './firmware-fixture';

describe('parseTelemetry (mirror of pack_telemetry offsets)', () => {
  it('reads every field little-endian at its exact offset', () => {
    // Distinct values per field so a swapped offset would fail.
    const sample: TelemetrySample = {
      uptimeMs: 0x01020304,
      scanCount: 0x05060708,
      reportCount: 0x090a0b0c,
      activeLayers: 0b0000_0000_0000_0101, // layers 0 and 2
      scanRateHz: 1000,
      lastProcUs: 0x0d0e0f10,
      battery: 77,
      rssi: 0xff,
      connection: 6,
    };
    const t = parseTelemetry(packTelemetry(sample));
    expect(t).toEqual({
      uptimeMs: 0x01020304,
      scanCount: 0x05060708,
      reportCount: 0x090a0b0c,
      activeLayers: 0b101,
      scanRateHz: 1000,
      lastProcUs: 0x0d0e0f10,
      battery: 77,
      rssi: null, // 0xFF sentinel decoded to "unavailable"
      connection: 6,
    });
  });

  it('decodes a high uptime (top bit set) without sign error', () => {
    const t = parseTelemetry(packTelemetry({ ...DEFAULT_TELEMETRY, uptimeMs: 0xffff_fffe }));
    expect(t.uptimeMs).toBe(0xffff_fffe);
  });

  it('maps the on-USB defaults: battery 100, RSSI unavailable', () => {
    const t = parseTelemetry(packTelemetry());
    expect(t.battery).toBe(100);
    expect(t.rssi).toBeNull();
    expect(t.connection).toBe(0); // USB
    expect(t.scanRateHz).toBe(1000);
  });
});

describe('activeLayerList', () => {
  it('expands the bitmask into active layer indices', () => {
    expect(activeLayerList(0b0001)).toEqual([0]);
    expect(activeLayerList(0b0101)).toEqual([0, 2]);
    expect(activeLayerList(0)).toEqual([]);
  });
});

describe('GET_TELEMETRY through the codec', () => {
  it('returns the device snapshot with Status::Ok', () => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.GetTelemetry, 0x11)));
    expect(reply.status).toBe(Status.Ok);
    const t = parseTelemetry(reply.payload);
    expect(t.scanRateHz).toBe(1000);
    expect(t.battery).toBe(100);
  });
});
