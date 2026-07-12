// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  hsvToRgb,
  packZoneArgs,
  parseModeList,
  parseRgbState,
  parseZone,
  parseZones,
  rgbModeLabel,
  withZoneFlag,
  zoneEnabled,
  zoneLabel,
  zoneLinked,
  ZONE_FLAG_ENABLED,
  ZONE_FLAG_LINKED,
  ZONE_SYNC_NONE,
  type RgbState,
  type ZoneState,
} from './rgb';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  DEFAULT_RGB_STATE,
  DEFAULT_ZONES,
  MODE_COUNT,
  MODE_IDS,
  ZONE_CAP,
  ZONE_COUNT,
  type RgbStateSample,
  type ZoneSample,
  createFakeDevice,
  fakeFirmwareHandle,
  packRgbModeList,
  packRgbState,
  packZone,
  packZones,
} from './firmware-fixture';

describe('parseRgbState (mirror of pack_rgb_state)', () => {
  it('reads mode, hsv, brightness, enabled, the LED count (u16 LE), speed and indicators', () => {
    const sample: RgbStateSample = {
      mode: 1,
      hue: 200,
      sat: 180,
      val: 240,
      brightness: 128,
      enabled: true,
      ledCount: 105,
      speed: 90,
      indicators: false,
    };
    const expected: RgbState = { ...sample };
    expect(parseRgbState(packRgbState(sample))).toEqual(expected);
  });

  it('decodes the disabled flag as a boolean', () => {
    const state = parseRgbState(packRgbState({ ...DEFAULT_RGB_STATE, enabled: false }));
    expect(state.enabled).toBe(false);
  });

  it('keeps the full 0..=255 brightness range (default 128 exceeds the 84 cap)', () => {
    const state = parseRgbState(packRgbState());
    expect(state.brightness).toBe(128);
  });
});

describe('parseZones (mirror of GET_ZONES)', () => {
  it('reads the zone count and capacity', () => {
    expect(parseZones(packZones())).toEqual({ zoneCount: ZONE_COUNT, zoneCap: ZONE_CAP });
  });
});

describe('parseZone (mirror of pack_zone)', () => {
  it('reads id, flags, mode, hsv, brightness, speed, the range (u16 LE) and the sync byte', () => {
    const sample: ZoneSample = {
      flags: ZONE_FLAG_ENABLED,
      mode: 4,
      hue: 30,
      sat: 200,
      val: 220,
      brightness: 100,
      speed: 60,
      start: 83,
      count: 11,
      syncTo: 2,
    };
    expect(parseZone(packZone(1, sample))).toEqual({ id: 1, ...sample });
  });

  it('decodes the enabled and linked flag bits', () => {
    const linked = parseZone(
      packZone(0, { ...DEFAULT_ZONES[0], flags: ZONE_FLAG_ENABLED | ZONE_FLAG_LINKED }),
    );
    expect(zoneEnabled(linked)).toBe(true);
    expect(zoneLinked(linked)).toBe(true);
    const off = parseZone(packZone(0, { ...DEFAULT_ZONES[0], flags: 0 }));
    expect(zoneEnabled(off)).toBe(false);
    expect(zoneLinked(off)).toBe(false);
  });

  it('decodes a zero-filled (older-firmware) sync byte as not-synced, not zone 0', () => {
    // An older firmware zero-fills its GET_ZONE reply; the biased wire encoding makes a
    // zero byte 12 mean "not synced", so the app must not read it as "synced to zone 0".
    const zeroFilled = new Uint8Array(13);
    expect(parseZone(zeroFilled).syncTo).toBe(ZONE_SYNC_NONE);
  });
});

describe('zone helpers', () => {
  it('labels the v1 zones and falls back for unknown ids', () => {
    expect(zoneLabel(0)).toBe('Keys');
    expect(zoneLabel(1)).toBe('Right');
    expect(zoneLabel(2)).toBe('Left');
    expect(zoneLabel(9)).toBe('Zone 9');
  });

  it('withZoneFlag sets and clears a bit, preserving the others', () => {
    expect(withZoneFlag(ZONE_FLAG_ENABLED, ZONE_FLAG_LINKED, true)).toBe(
      ZONE_FLAG_ENABLED | ZONE_FLAG_LINKED,
    );
    expect(withZoneFlag(ZONE_FLAG_ENABLED | ZONE_FLAG_LINKED, ZONE_FLAG_LINKED, false)).toBe(
      ZONE_FLAG_ENABLED,
    );
  });
});

describe('parseModeList', () => {
  it('pages [total, page_len, ids…] reassemble into the full firmware MODE_IDS', () => {
    const ids: number[] = [];
    let total = 0;
    // Page from where we left off until we have every id (the loop the client runs).
    for (;;) {
      const page = parseModeList(packRgbModeList(ids.length));
      total = page.total;
      ids.push(...page.ids);
      if (page.ids.length === 0 || ids.length >= page.total) break;
    }
    expect(total).toBe(MODE_COUNT);
    expect(ids).toEqual(MODE_IDS);
  });
});

describe('rgbModeLabel', () => {
  it('labels the known modes (effects and reactive) and falls back for unknown ids', () => {
    expect(rgbModeLabel(0)).toBe('Solid');
    expect(rgbModeLabel(1)).toBe('Breathing');
    expect(rgbModeLabel(2)).toBe('Rainbow');
    expect(rgbModeLabel(6)).toBe('Cycle Left-Right');
    expect(rgbModeLabel(9)).toBe('Raindrops');
    expect(rgbModeLabel(10)).toBe('Solid Reactive');
    expect(rgbModeLabel(13)).toBe('Splash');
    expect(rgbModeLabel(14)).toBe('Reactive Rainbow');
    expect(rgbModeLabel(99)).toBe('Mode 99');
  });
});

describe('hsvToRgb (port of the firmware integer conversion)', () => {
  it('returns grey when saturation is zero', () => {
    expect(hsvToRgb(0, 0, 200)).toEqual({ r: 200, g: 200, b: 200 });
  });

  it('returns pure red at hue 0, full saturation and value', () => {
    expect(hsvToRgb(0, 255, 255)).toEqual({ r: 255, g: 0, b: 0 });
  });

  it('keeps every channel an integer within 0..=255 across a hue sweep', () => {
    for (let h = 0; h <= 255; h += 17) {
      const { r, g, b } = hsvToRgb(h, 255, 255);
      for (const channel of [r, g, b]) {
        expect(Number.isInteger(channel)).toBe(true);
        expect(channel).toBeGreaterThanOrEqual(0);
        expect(channel).toBeLessThanOrEqual(255);
      }
    }
  });
});

describe('RGB dispatch through the codec', () => {
  it('set mode / hsv / brightness / enabled / speed / indicators are observed by get state', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    expect(run(Cmd.RgbSetMode, 1, [1]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetHsv, 2, [10, 20, 30]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetBrightness, 3, [200]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetEnabled, 4, [0]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetSpeed, 5, [64]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetIndicators, 6, [0]).status).toBe(Status.Ok);

    const state = parseRgbState(run(Cmd.RgbGetState, 7).payload);
    expect(state).toMatchObject({
      mode: 1,
      hue: 10,
      sat: 20,
      val: 30,
      brightness: 200,
      enabled: false,
      ledCount: 105,
      speed: 64,
      indicators: false,
    });
  });

  it('rejects an out-of-range mode and a non-boolean enable / indicators with BadArg', () => {
    const device = createFakeDevice();
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.RgbSetMode, 1, [MODE_COUNT]), device))
        .status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.RgbSetEnabled, 2, [2]), device)).status,
    ).toBe(Status.BadArg);
    expect(
      decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.RgbSetIndicators, 3, [2]), device)).status,
    ).toBe(Status.BadArg);
  });

  it('lists the available modes across reply pages', () => {
    const ids: number[] = [];
    // LIST_MODES pages by start offset; walk the pages as the client does.
    for (;;) {
      const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.RgbListModes, 0, [ids.length])));
      expect(reply.status).toBe(Status.Ok);
      const page = parseModeList(reply.payload);
      ids.push(...page.ids);
      if (page.ids.length === 0 || ids.length >= page.total) break;
    }
    expect(ids).toEqual(MODE_IDS);
  });

  it('get zones reports the count and capacity', () => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(Cmd.RgbGetZones, 0)));
    expect(reply.status).toBe(Status.Ok);
    expect(parseZones(reply.payload)).toEqual({ zoneCount: ZONE_COUNT, zoneCap: ZONE_CAP });
  });

  it('set zone / get zone round-trips flags, effect, color, brightness and speed', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    const zone: ZoneState = {
      id: 1,
      flags: ZONE_FLAG_ENABLED,
      mode: 0,
      hue: 170,
      sat: 255,
      val: 200,
      brightness: 90,
      speed: 40,
      start: 83,
      count: 11,
      syncTo: ZONE_SYNC_NONE,
    };
    expect(run(Cmd.RgbSetZone, 1, packZoneArgs(zone)).status).toBe(Status.Ok);
    expect(parseZone(run(Cmd.RgbGetZone, 2, [1]).payload)).toEqual(zone);
  });

  it('rejects an out-of-range zone id or mode (BadArg)', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));
    const base: ZoneState = { id: 0, ...DEFAULT_ZONES[0] };

    expect(run(Cmd.RgbGetZone, 1, [ZONE_CAP]).status).toBe(Status.BadArg);
    expect(run(Cmd.RgbSetZone, 2, packZoneArgs({ ...base, id: ZONE_CAP })).status).toBe(
      Status.BadArg,
    );
    expect(run(Cmd.RgbSetZone, 3, packZoneArgs({ ...base, mode: MODE_COUNT })).status).toBe(
      Status.BadArg,
    );
  });

  it('set zone range resizes, then rejects a past-chain or overlapping range', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    // [id, start:u16le, count:u16le]. The default zones tile the chain, so first
    // shrink Keys (0..83 -> 0..40) to free space, then carve the spare zone 3 into it.
    expect(run(Cmd.RgbSetZoneRange, 1, [0, 0, 0, 40, 0]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetZoneRange, 2, [3, 40, 0, 20, 0]).status).toBe(Status.Ok);
    expect(parseZone(run(Cmd.RgbGetZone, 3, [3]).payload)).toMatchObject({ start: 40, count: 20 });
    // 100 + 20 = 120 > 105 (past the chain).
    expect(run(Cmd.RgbSetZoneRange, 4, [3, 100, 0, 20, 0]).status).toBe(Status.BadArg);
    // 30..50 overlaps the (still enabled) Keys zone 0..40.
    expect(run(Cmd.RgbSetZoneRange, 5, [3, 30, 0, 20, 0]).status).toBe(Status.BadArg);
    // A zero-count range is inert and overlaps nothing, even inside another zone.
    expect(run(Cmd.RgbSetZoneRange, 6, [3, 10, 0, 0, 0]).status).toBe(Status.Ok);
  });

  it('rejects re-enabling a zone whose range another was resized over while it was disabled', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    // [id, flags, mode, h, s, v, bright, speed]. Disable Keys (zone 0), leaving its
    // 0..83 range reserved but dark.
    expect(run(Cmd.RgbSetZone, 1, [0, 0, 2, 0, 0, 0, 0, 0]).status).toBe(Status.Ok);
    // Resize the spare zone 3 over Keys' now-dark range — allowed while Keys is off.
    expect(run(Cmd.RgbSetZoneRange, 2, [3, 0, 0, 40, 0]).status).toBe(Status.Ok);
    // Re-enabling Keys (0..83) would now overlap the lit zone 3 (0..40): rejected.
    expect(run(Cmd.RgbSetZone, 3, [0, ZONE_FLAG_ENABLED, 2, 0, 0, 0, 0, 0]).status).toBe(
      Status.BadArg,
    );
    // The rejected enable changed nothing — Keys stays disabled.
    expect(zoneEnabled(parseZone(run(Cmd.RgbGetZone, 4, [0]).payload))).toBe(false);
  });

  it('accepts a direct frame chunk and rejects one past the chain', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    // [offset:u16le, len, rgb…] — two pixels at offset 0.
    expect(run(Cmd.RgbDirect, 1, [0, 0, 2, 255, 0, 0, 0, 255, 0]).status).toBe(Status.Ok);
    // offset 104 + 2 pixels = 106 > 105.
    expect(run(Cmd.RgbDirect, 2, [104, 0, 2, 1, 2, 3, 4, 5, 6]).status).toBe(Status.BadArg);
  });

  it('set zone sync links a zone, reflects it in get zone, and clears with 0xFF', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    // [id, target] — Left (2) mirrors Right (1).
    expect(run(Cmd.RgbSetZoneSync, 1, [2, 1]).status).toBe(Status.Ok);
    expect(parseZone(run(Cmd.RgbGetZone, 2, [2]).payload).syncTo).toBe(1);
    // 0xFF clears the link.
    expect(run(Cmd.RgbSetZoneSync, 3, [2, ZONE_SYNC_NONE]).status).toBe(Status.Ok);
    expect(parseZone(run(Cmd.RgbGetZone, 4, [2]).payload).syncTo).toBe(ZONE_SYNC_NONE);
  });

  it('set zone sync rejects a bad id/target, a self-sync, and a sync cycle', () => {
    const device = createFakeDevice();
    const run = (cmd: number, seq: number, payload?: number[]) =>
      decodeReply(fakeFirmwareHandle(encodeRequest(cmd, seq, payload), device));

    expect(run(Cmd.RgbSetZoneSync, 1, [ZONE_CAP, 1]).status).toBe(Status.BadArg); // bad id
    expect(run(Cmd.RgbSetZoneSync, 2, [1, ZONE_CAP]).status).toBe(Status.BadArg); // bad target
    expect(run(Cmd.RgbSetZoneSync, 3, [1, 1]).status).toBe(Status.BadArg); // self-sync
    // 1 -> 2 is fine; 2 -> 1 would close a cycle and is rejected.
    expect(run(Cmd.RgbSetZoneSync, 4, [1, 2]).status).toBe(Status.Ok);
    expect(run(Cmd.RgbSetZoneSync, 5, [2, 1]).status).toBe(Status.BadArg);
  });
});
