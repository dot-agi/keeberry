// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { readFullConfig, writeFullConfig, type ConfigClient, type MatrixDims } from './snapshot';
import {
  COMBO_FLAG_IN_ORDER,
  LAYERS,
  MAX_MACRO,
  MAX_SOCD,
  NUM_COLS,
  NUM_ROWS,
  ZONE_FLAG_ENABLED,
  ZONE_FLAG_LINKED,
  ZONE_SYNC_NONE,
  createFakeDevice,
  fakeFirmwareHandle,
  type FakeDevice,
} from './firmware-fixture';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import {
  encodeGetKeycodeArgs,
  encodeSetKeycodeArgs,
  encodeSetLayerConfigArgs,
  parseKeycodeReply,
  parseLayerConfig,
} from './keymap';
import { encodeSetKroArgs, parseKro } from './hidkro';
import { encodeSetDebounceArgs, encodeSetTuningArgs, parseDebounce, parseTuning } from './config';
import { packZoneArgs, parseRgbState, parseZone, parseZones } from './rgb';
import {
  CLEAR_ALL,
  encodeComboSetArgs,
  encodeIndexArg,
  encodeLeaderSetArgs,
  encodeOverrideSetArgs,
  encodeSocdSetArgs,
  encodeTapdanceSetArgs,
  parseBehaviorInfo,
  parseCombo,
  parseLeader,
  parseOverride,
  parseSocdPair,
  parseTapdance,
  parseTimedInfo,
} from './behavior';
import {
  encodeGetFeaturesArgs,
  encodeSetFeatureEnabledArgs,
  parseFeaturesPage,
  type FeatureRecord,
} from './features';
import {
  encodeMacroGetStepArgs,
  encodeMacroSetStepArgs,
  parseMacroInfo,
  parseMacroStep,
} from './macro';

const DIMS: MatrixDims = { rows: NUM_ROWS, cols: NUM_COLS, layers: LAYERS };

/**
 * A {@link ConfigClient} backed by the stateful firmware fixture: each method
 * encodes its args, runs the real codec through `fakeFirmwareHandle` against a
 * mutable {@link FakeDevice}, and parses the reply — exactly the wire path
 * {@link KcpClient} takes, so the snapshot is exercised end-to-end.
 */
function fixtureConfigClient(device: FakeDevice): ConfigClient {
  const tx = (cmd: number, payload?: number[]) => {
    const reply = decodeReply(fakeFirmwareHandle(encodeRequest(cmd, 0, payload), device));
    if (reply.status !== Status.Ok) {
      throw new Error(`fixture cmd 0x${cmd.toString(16)} failed with status ${reply.status}`);
    }
    return reply;
  };
  return {
    getKeycode: async (l, r, c) =>
      parseKeycodeReply(tx(Cmd.GetKeycode, encodeGetKeycodeArgs(l, r, c)).payload),
    setKeycode: async (l, r, c, kc) => {
      tx(Cmd.SetKeycode, encodeSetKeycodeArgs(l, r, c, kc));
    },
    getKro: async () => parseKro(tx(Cmd.GetKro).payload),
    setKro: async (enabled) => {
      tx(Cmd.SetKro, encodeSetKroArgs(enabled));
    },
    getLayerConfig: async () => parseLayerConfig(tx(Cmd.GetLayerConfig).payload),
    setLayerConfig: async (cfg) => {
      tx(Cmd.SetLayerConfig, encodeSetLayerConfigArgs(cfg));
    },
    getDebounce: async () => parseDebounce(tx(Cmd.ConfigGetDebounce).payload),
    setDebounce: async (cfg) => {
      tx(Cmd.ConfigSetDebounce, encodeSetDebounceArgs(cfg));
    },
    getTuning: async () => parseTuning(tx(Cmd.ConfigGetTuning).payload),
    setTuning: async (cfg) => {
      tx(Cmd.ConfigSetTuning, encodeSetTuningArgs(cfg));
    },
    rgbGetState: async () => parseRgbState(tx(Cmd.RgbGetState).payload),
    rgbSetMode: async (mode) => {
      tx(Cmd.RgbSetMode, [mode & 0xff]);
    },
    rgbSetHsv: async (h, s, v) => {
      tx(Cmd.RgbSetHsv, [h & 0xff, s & 0xff, v & 0xff]);
    },
    rgbSetBrightness: async (value) => {
      tx(Cmd.RgbSetBrightness, [value & 0xff]);
    },
    rgbSetEnabled: async (enabled) => {
      tx(Cmd.RgbSetEnabled, [enabled ? 1 : 0]);
    },
    rgbSetSpeed: async (speed) => {
      tx(Cmd.RgbSetSpeed, [speed & 0xff]);
    },
    rgbSetIndicators: async (enabled) => {
      tx(Cmd.RgbSetIndicators, [enabled ? 1 : 0]);
    },
    rgbGetZones: async () => parseZones(tx(Cmd.RgbGetZones).payload),
    rgbGetZone: async (id) => parseZone(tx(Cmd.RgbGetZone, [id & 0xff]).payload),
    rgbSetZone: async (zone) => {
      tx(Cmd.RgbSetZone, packZoneArgs(zone));
    },
    rgbSetZoneRange: async (id, start, count) => {
      tx(Cmd.RgbSetZoneRange, [
        id & 0xff,
        start & 0xff,
        (start >> 8) & 0xff,
        count & 0xff,
        (count >> 8) & 0xff,
      ]);
    },
    rgbSetZoneSync: async (id, target) => {
      tx(Cmd.RgbSetZoneSync, [id & 0xff, target & 0xff]);
    },
    behaviorInfo: async () => parseBehaviorInfo(tx(Cmd.BehaviorInfo).payload),
    socdGet: async (i) => parseSocdPair(tx(Cmd.SocdGet, encodeIndexArg(i)).payload),
    socdSet: async (i, pair) => {
      tx(Cmd.SocdSet, encodeSocdSetArgs(i, pair));
    },
    socdClearAll: async () => {
      tx(Cmd.SocdClear, encodeIndexArg(CLEAR_ALL));
    },
    overrideGet: async (i) => parseOverride(tx(Cmd.OverrideGet, encodeIndexArg(i)).payload),
    overrideSet: async (i, override) => {
      tx(Cmd.OverrideSet, encodeOverrideSetArgs(i, override));
    },
    overrideClearAll: async () => {
      tx(Cmd.OverrideClear, encodeIndexArg(CLEAR_ALL));
    },
    timedInfo: async () => parseTimedInfo(tx(Cmd.TimedInfo).payload),
    tapdanceGet: async (i) => parseTapdance(tx(Cmd.TapdanceGet, encodeIndexArg(i)).payload),
    tapdanceSet: async (i, td) => {
      tx(Cmd.TapdanceSet, encodeTapdanceSetArgs(i, td));
    },
    tapdanceClearAll: async () => {
      tx(Cmd.TapdanceClear, encodeIndexArg(CLEAR_ALL));
    },
    comboGet: async (i) => parseCombo(tx(Cmd.ComboGet, encodeIndexArg(i)).payload),
    comboSet: async (i, combo) => {
      tx(Cmd.ComboSet, encodeComboSetArgs(i, combo));
    },
    comboClearAll: async () => {
      tx(Cmd.ComboClear, encodeIndexArg(CLEAR_ALL));
    },
    leaderGet: async (i) => parseLeader(tx(Cmd.LeaderGet, encodeIndexArg(i)).payload),
    leaderSet: async (i, leader) => {
      tx(Cmd.LeaderSet, encodeLeaderSetArgs(i, leader));
    },
    leaderClearAll: async () => {
      const { maxLeader } = parseTimedInfo(tx(Cmd.TimedInfo).payload);
      for (let i = 0; i < maxLeader; i += 1) {
        tx(Cmd.LeaderSet, encodeLeaderSetArgs(i, { seq: [], action: 0 }));
      }
    },
    macroInfo: async () => parseMacroInfo(tx(Cmd.MacroInfo).payload),
    macroGetStep: async (m, s) =>
      parseMacroStep(tx(Cmd.MacroGetStep, encodeMacroGetStepArgs(m, s)).payload),
    macroSetStep: async (m, s, ev) => {
      tx(Cmd.MacroSetStep, encodeMacroSetStepArgs(m, s, ev));
    },
    macroClearAll: async () => {
      tx(Cmd.MacroClear, encodeIndexArg(CLEAR_ALL));
    },
    listFeatures: async () => {
      const records: FeatureRecord[] = [];
      for (;;) {
        const page = parseFeaturesPage(
          tx(Cmd.GetFeatures, encodeGetFeaturesArgs(records.length)).payload,
        );
        records.push(...page.records);
        if (page.records.length === 0 || records.length >= page.count) {
          return records;
        }
      }
    },
    setFeatureEnabled: async (id, enabled) => {
      tx(Cmd.SetFeatureEnabled, encodeSetFeatureEnabledArgs(id, enabled));
    },
  };
}

/** A fake device seeded with a distinctive value in every group. */
function seededDevice(): FakeDevice {
  const d = createFakeDevice();
  for (let l = 0; l < LAYERS; l += 1) {
    for (let r = 0; r < NUM_ROWS; r += 1) {
      for (let c = 0; c < NUM_COLS; c += 1) {
        d.keymap[l][r][c] = (l * 100 + r * 10 + c) & 0xffff;
      }
    }
  }
  d.nkro = true;
  d.rgb = {
    mode: 1,
    hue: 10,
    sat: 200,
    val: 150,
    brightness: 84,
    enabled: false,
    ledCount: 105,
    speed: 64,
    indicators: true,
  };
  d.socd[0] = { a: 4, b: 7, mode: 1 };
  d.socd[3] = { a: 0x1e, b: 0x1f, mode: 2 };
  d.overrides[1] = {
    trigger: 4,
    triggerMods: 2,
    replacement: 5,
    replacementMods: 0,
    layerMask: 1,
    enabled: true,
  };
  d.tapDance[0] = { tap: 4, hold: 5, double: 6, termMs: 200 };
  d.combos[2] = { keys: [4, 5, 0, 0], len: 2, action: 6, termMs: 50, flags: COMBO_FLAG_IN_ORDER };
  d.macros[0].steps[0] = { kc: 4, down: true, delayMs: 10 };
  d.macros[0].steps[1] = { kc: 4, down: false, delayMs: 20 };
  d.macros[0].len = 2;
  return d;
}

/**
 * Layer on the sections added with the persisted-config groups (debounce, tuning,
 * leader, layer config, RGB zones + indicator overlay, feature enables) on top of the
 * base {@link seededDevice}, with non-default values so a round-trip that silently
 * dropped them would fail. The zone table deliberately includes a disabled slot whose
 * range overlaps a lit one and a zone synced to another, to exercise the safe-order
 * zone rebuild.
 */
function fullySeededDevice(): FakeDevice {
  const d = seededDevice();
  d.layerConfig = { defaultLayer: 2, triEnabled: true, triL1: 1, triL2: 3, triL3: 5 };
  d.debounce = { algorithm: 1, interval: 8 };
  d.tuning = {
    autoShiftEnabled: true,
    autoShiftTimeoutMs: 222,
    leaderTimeoutMs: 333,
    tapHoldTermMs: 180,
    permissiveHold: true,
    holdOnOtherKeyPress: false,
    retroTapping: true,
    chordalHold: false,
    quickTapTermMs: 0,
  };
  d.rgb.indicators = false;
  d.zones = [
    // Lit, independent.
    {
      flags: ZONE_FLAG_ENABLED,
      mode: 5,
      hue: 10,
      sat: 200,
      val: 150,
      brightness: 80,
      speed: 40,
      start: 0,
      count: 40,
      syncTo: ZONE_SYNC_NONE,
    },
    // Lit, linked.
    {
      flags: ZONE_FLAG_ENABLED | ZONE_FLAG_LINKED,
      mode: 2,
      hue: 0,
      sat: 255,
      val: 255,
      brightness: 128,
      speed: 128,
      start: 40,
      count: 10,
      syncTo: ZONE_SYNC_NONE,
    },
    // Disabled, range overlaps zone 1 (legal because it is blanked).
    {
      flags: 0,
      mode: 7,
      hue: 128,
      sat: 128,
      val: 128,
      brightness: 64,
      speed: 64,
      start: 45,
      count: 10,
      syncTo: ZONE_SYNC_NONE,
    },
    // Lit, synced to zone 0.
    {
      flags: ZONE_FLAG_ENABLED,
      mode: 3,
      hue: 200,
      sat: 100,
      val: 200,
      brightness: 100,
      speed: 100,
      start: 60,
      count: 10,
      syncTo: 0,
    },
  ];
  d.leader[0] = { seq: [4, 5, 6, 0, 0], len: 3, action: 7 };
  d.leader[4] = { seq: [8, 0, 0, 0, 0], len: 1, action: 9 };
  d.featuresEnabled[5] = false; // Repeat Key off (a toggleable feature).
  return d;
}

describe('readFullConfig', () => {
  it('reads the complete editable device state across every group', async () => {
    const cfg = await readFullConfig(fixtureConfigClient(seededDevice()), DIMS);

    expect(cfg.keymap[0][0][0]).toBe(0);
    expect(cfg.keymap[1][5][14]).toBe(164);
    expect(cfg.nkro).toBe(true);
    expect(cfg.rgb).toEqual({
      mode: 1,
      hue: 10,
      sat: 200,
      val: 150,
      brightness: 84,
      enabled: false,
      speed: 64,
      indicators: true,
    });
    expect(cfg.socd[0]).toEqual({ a: 4, b: 7, mode: 1 });
    expect(cfg.socd[3]).toEqual({ a: 0x1e, b: 0x1f, mode: 2 });
    expect(cfg.socd[1]).toBeNull();
    expect(cfg.overrides[1]).toEqual({
      trigger: 4,
      triggerMods: 2,
      replacement: 5,
      replacementMods: 0,
      layerMask: 1,
      enabled: true,
    });
    expect(cfg.tapDance[0]).toEqual({ tap: 4, hold: 5, double: 6, termMs: 200 });
    expect(cfg.combos[2]).toEqual({
      keys: [4, 5],
      action: 6,
      termMs: 50,
      mustHold: false,
      mustTap: false,
      inOrder: true,
    });
    expect(cfg.macros[0]).toEqual([
      { keycode: 4, down: true, delayMs: 10 },
      { keycode: 4, down: false, delayMs: 20 },
    ]);
    expect(cfg.macros[1]).toEqual([]);
    // Table lengths track the device's advertised capacities.
    expect(cfg.socd).toHaveLength(MAX_SOCD);
    expect(cfg.macros).toHaveLength(MAX_MACRO);
  });

  it('reads the persisted-config sections (layer, debounce, tuning, zones, leader, features)', async () => {
    const cfg = await readFullConfig(fixtureConfigClient(fullySeededDevice()), DIMS);

    expect(cfg.layerConfig).toEqual({
      defaultLayer: 2,
      triEnabled: true,
      triL1: 1,
      triL2: 3,
      triL3: 5,
    });
    expect(cfg.debounce).toEqual({ algorithm: 1, interval: 8 });
    expect(cfg.tuning.autoShiftEnabled).toBe(true);
    expect(cfg.tuning.leaderTimeoutMs).toBe(333);
    expect(cfg.tuning.quickTapTermMs).toBe(0);
    expect(cfg.rgb.indicators).toBe(false);
    // The full ZONE_CAP table is captured, ids and all.
    expect(cfg.zones).toHaveLength(4);
    expect(cfg.zones[0]).toMatchObject({ id: 0, start: 0, count: 40 });
    expect(cfg.zones[2]).toMatchObject({ id: 2, flags: 0, start: 45, count: 10 });
    expect(cfg.zones[3]).toMatchObject({ id: 3, syncTo: 0 });
    expect(cfg.leaders[0]).toEqual({ seq: [4, 5, 6], action: 7 });
    expect(cfg.leaders[4]).toEqual({ seq: [8], action: 9 });
    expect(cfg.leaders[1]).toBeNull();
    // The feature registry round-trips its enable bitmap (Repeat Key off, rest on).
    expect(cfg.features.find((f) => f.id === 5)?.enabled).toBe(false);
    expect(cfg.features.find((f) => f.id === 0)?.enabled).toBe(true);
  });
});

describe('writeFullConfig', () => {
  it('writes a config back so a re-read is identical (round-trip)', async () => {
    const source = await readFullConfig(fixtureConfigClient(seededDevice()), DIMS);

    const target = fixtureConfigClient(createFakeDevice());
    await writeFullConfig(target, source);

    expect(await readFullConfig(target, DIMS)).toEqual(source);
  });

  it('round-trips the expanded sections including the safe-order zone rebuild', async () => {
    const source = await readFullConfig(fixtureConfigClient(fullySeededDevice()), DIMS);

    const target = fixtureConfigClient(createFakeDevice());
    await writeFullConfig(target, source);
    const restored = await readFullConfig(target, DIMS);

    expect(restored).toEqual(source);
    // Spot-check the trickier sections survived rather than silently resetting.
    expect(restored.layerConfig).toEqual(source.layerConfig);
    expect(restored.debounce).toEqual(source.debounce);
    expect(restored.tuning).toEqual(source.tuning);
    expect(restored.rgb.indicators).toBe(false);
    expect(restored.zones[2]).toMatchObject({ flags: 0, start: 45, count: 10 });
    expect(restored.zones[3].syncTo).toBe(0);
    expect(restored.leaders[0]).toEqual({ seq: [4, 5, 6], action: 7 });
    expect(restored.features.find((f) => f.id === 5)?.enabled).toBe(false);
  });

  it('overwrites exactly, clearing slots and macros absent from the written config', async () => {
    // The target starts seeded; writing an all-empty config must clear it back out.
    const target = fixtureConfigClient(fullySeededDevice());
    const empty = await readFullConfig(fixtureConfigClient(createFakeDevice()), DIMS);

    await writeFullConfig(target, empty);

    expect(await readFullConfig(target, DIMS)).toEqual(empty);
  });
});
