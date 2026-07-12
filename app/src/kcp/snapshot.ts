// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Full-config snapshot: read the **complete** editable device state over kcp into
 * one plain object, and write it back. This is the host-side mirror of the
 * firmware's flash blob (`config.rs`): the same set of groups `CONFIG.SAVE`
 * persists — keymap, NKRO, the layer config (default `DF` layer + tri-layer), the
 * matrix debounce config, the timed-engine tunables, the RGB state (base effect,
 * indicator overlay and the zone table), the behaviour tables (SOCD, key overrides,
 * tap-dance, combos, leader sequences), macros and the feature-enable bitmap — but
 * carried live over the kcp group wrappers instead of serialised to flash.
 *
 * It backs config export/import (a downloadable JSON file) and persist-across-
 * flash (a `localStorage` backup restored after a re-flash). Capacities and matrix
 * dimensions are read from the device, never hard-coded, so a snapshot tracks
 * whatever the connected firmware reports. Writing clears each table first, then
 * applies the present slots, so the result is an exact overwrite — exactly how the
 * firmware's `restore_blob` rebuilds RAM from a blob.
 */
import type { KcpClient } from './client';
import type { Combo, KeyOverride, Leader, SocdPair, TapDance } from './behavior';
import type { DebounceConfig, TuningConfig } from './config';
import type { FeatureRecord } from './features';
import type { LayerConfig } from './keymap';
import type { MacroStep } from './macro';
import {
  ZONE_FLAG_ENABLED,
  ZONE_SYNC_NONE,
  withZoneFlag,
  zoneEnabled,
  zoneSynced,
  type RgbState,
  type ZoneState,
} from './rgb';

/**
 * The writable scalar RGB state (`RGB.GET_STATE`): the base effect plus the
 * status-indicator overlay flag. The read-only `ledCount` is device-derived, so
 * omitted; the zone table is a separate {@link FullConfig.zones} group.
 */
export interface RgbConfig {
  mode: number;
  hue: number;
  sat: number;
  val: number;
  brightness: number;
  enabled: boolean;
  speed: number;
  /** Whether the firmware status-indicator overlay is drawn (persisted from schema v7). */
  indicators: boolean;
}

/**
 * The complete editable device state — every group `CONFIG.SAVE` persists. Tables
 * are fixed-length arrays (one entry per device slot, `null` when empty); each
 * macro is the array of its active steps (empty when the macro is unused).
 */
export interface FullConfig {
  /** `[layer][row][col]` raw `u16` keycodes. */
  keymap: number[][][];
  /** Rollover mode (`true` = NKRO, `false` = boot 6KRO). */
  nkro: boolean;
  /** The default (base) layer and the tri-layer rule. */
  layerConfig: LayerConfig;
  /** The matrix debounce algorithm + interval. */
  debounce: DebounceConfig;
  /** The timed-engine runtime tunables (auto-shift, leader timeout, tap-hold, quick-tap). */
  tuning: TuningConfig;
  /** The writable scalar RGB state (base effect + indicator overlay flag). */
  rgb: RgbConfig;
  /** The full RGB zone table, one entry per device zone slot. */
  zones: ZoneState[];
  /** SOCD table, one entry per slot (`null` = empty). */
  socd: (SocdPair | null)[];
  /** Key-override table, one entry per slot (`null` = empty). */
  overrides: (KeyOverride | null)[];
  /** Tap-dance table, one entry per slot (`null` = empty). */
  tapDance: (TapDance | null)[];
  /** Combo table, one entry per slot (`null` = empty). */
  combos: (Combo | null)[];
  /** Leader-sequence table, one entry per slot (`null` = empty). */
  leaders: (Leader | null)[];
  /** Macro table; each entry is that macro's active steps (empty when unused). */
  macros: MacroStep[][];
  /** Every registered feature and its runtime enable (the persisted enable bitmap). */
  features: FeatureRecord[];
}

/** Matrix dimensions the keymap walk needs (from the INFO device descriptor). */
export interface MatrixDims {
  rows: number;
  cols: number;
  layers: number;
}

/**
 * The kcp client surface the snapshot uses — exactly the read/write group
 * wrappers, no more. Pinning it to a structural subset of {@link KcpClient} keeps
 * the dependency explicit and lets tests drive it with a fixture-backed stand-in.
 */
export type ConfigClient = Pick<
  KcpClient,
  | 'getKeycode'
  | 'setKeycode'
  | 'getKro'
  | 'setKro'
  | 'getLayerConfig'
  | 'setLayerConfig'
  | 'getDebounce'
  | 'setDebounce'
  | 'getTuning'
  | 'setTuning'
  | 'rgbGetState'
  | 'rgbSetMode'
  | 'rgbSetHsv'
  | 'rgbSetBrightness'
  | 'rgbSetEnabled'
  | 'rgbSetSpeed'
  | 'rgbSetIndicators'
  | 'rgbGetZones'
  | 'rgbGetZone'
  | 'rgbSetZone'
  | 'rgbSetZoneRange'
  | 'rgbSetZoneSync'
  | 'behaviorInfo'
  | 'socdGet'
  | 'socdSet'
  | 'socdClearAll'
  | 'overrideGet'
  | 'overrideSet'
  | 'overrideClearAll'
  | 'timedInfo'
  | 'tapdanceGet'
  | 'tapdanceSet'
  | 'tapdanceClearAll'
  | 'comboGet'
  | 'comboSet'
  | 'comboClearAll'
  | 'leaderGet'
  | 'leaderSet'
  | 'leaderClearAll'
  | 'macroInfo'
  | 'macroGetStep'
  | 'macroSetStep'
  | 'macroClearAll'
  | 'listFeatures'
  | 'setFeatureEnabled'
>;

/** Keep the writable scalar fields from a full {@link RgbState} readback. */
function toRgbConfig(state: RgbState): RgbConfig {
  return {
    mode: state.mode,
    hue: state.hue,
    sat: state.sat,
    val: state.val,
    brightness: state.brightness,
    enabled: state.enabled,
    speed: state.speed,
    indicators: state.indicators,
  };
}

/**
 * Read the complete editable device state into a {@link FullConfig}. Walks the
 * keymap cell by cell (the firmware exposes no whole-layer dump), then reads NKRO,
 * the layer / debounce / tuning configs, the RGB state and zone table, and every
 * behaviour / leader / macro slot the device advertises via its `*_INFO` capacities,
 * plus the feature registry. Read-only — it issues no setters.
 */
export async function readFullConfig(client: ConfigClient, dims: MatrixDims): Promise<FullConfig> {
  const keymap: number[][][] = [];
  for (let layer = 0; layer < dims.layers; layer += 1) {
    const rows: number[][] = [];
    for (let row = 0; row < dims.rows; row += 1) {
      const cols: number[] = [];
      for (let col = 0; col < dims.cols; col += 1) {
        cols.push(await client.getKeycode(layer, row, col));
      }
      rows.push(cols);
    }
    keymap.push(rows);
  }

  const nkro = await client.getKro();
  const layerConfig = await client.getLayerConfig();
  const debounce = await client.getDebounce();
  const tuning = await client.getTuning();
  const rgb = toRgbConfig(await client.rgbGetState());

  // The firmware persists the full ZONE_CAP zone table, so capture every slot.
  const zonesInfo = await client.rgbGetZones();
  const zones: ZoneState[] = [];
  for (let id = 0; id < zonesInfo.zoneCap; id += 1) {
    zones.push(await client.rgbGetZone(id));
  }

  const behavior = await client.behaviorInfo();
  const socd: (SocdPair | null)[] = [];
  for (let i = 0; i < behavior.maxSocd; i += 1) {
    socd.push(await client.socdGet(i));
  }
  const overrides: (KeyOverride | null)[] = [];
  for (let i = 0; i < behavior.maxOverrides; i += 1) {
    overrides.push(await client.overrideGet(i));
  }

  const timed = await client.timedInfo();
  const tapDance: (TapDance | null)[] = [];
  for (let i = 0; i < timed.maxTapDance; i += 1) {
    tapDance.push(await client.tapdanceGet(i));
  }
  const combos: (Combo | null)[] = [];
  for (let i = 0; i < timed.maxCombo; i += 1) {
    combos.push(await client.comboGet(i));
  }
  const leaders: (Leader | null)[] = [];
  for (let i = 0; i < timed.maxLeader; i += 1) {
    leaders.push(await client.leaderGet(i));
  }

  const macroInfo = await client.macroInfo();
  const macros: MacroStep[][] = [];
  for (let m = 0; m < macroInfo.maxMacro; m += 1) {
    // Step 0's readback carries the macro's active length, so read it once and
    // reuse it as the first step rather than fetching index 0 twice.
    const first = await client.macroGetStep(m, 0);
    const len = Math.min(first.len, macroInfo.maxSteps);
    const steps: MacroStep[] = [];
    for (let s = 0; s < len; s += 1) {
      const readback = s === 0 ? first : await client.macroGetStep(m, s);
      steps.push(readback.step);
    }
    macros.push(steps);
  }

  const features = await client.listFeatures();

  return {
    keymap,
    nkro,
    layerConfig,
    debounce,
    tuning,
    rgb,
    zones,
    socd,
    overrides,
    tapDance,
    combos,
    leaders,
    macros,
    features,
  };
}

/**
 * Restore the RGB zone table as an exact overwrite. The live setters reject a range
 * that overlaps an *enabled* zone, and a sync link that would close a cycle, so a
 * naive in-place apply can fail partway. Rebuild the whole table in a safe order:
 * unlink every zone, park them all disabled (a disabled zone is ignored by the
 * overlap check, so a disabled slot may legally overlap a lit one), lay down every
 * range while nothing is lit, then enable + style the lit zones (which are mutually
 * disjoint, so each off→on transition passes the overlap guard) and finally restore
 * the sync links.
 */
async function writeZones(client: ConfigClient, zones: ZoneState[]): Promise<void> {
  for (const zone of zones) {
    await client.rgbSetZoneSync(zone.id, ZONE_SYNC_NONE);
  }
  for (const zone of zones) {
    await client.rgbSetZone({ ...zone, flags: withZoneFlag(zone.flags, ZONE_FLAG_ENABLED, false) });
  }
  for (const zone of zones) {
    await client.rgbSetZoneRange(zone.id, zone.start, zone.count);
  }
  for (const zone of zones) {
    if (zoneEnabled(zone)) {
      await client.rgbSetZone(zone);
    }
  }
  for (const zone of zones) {
    if (zoneSynced(zone)) {
      await client.rgbSetZoneSync(zone.id, zone.syncTo);
    }
  }
}

/**
 * Write a {@link FullConfig} back into live device RAM, an exact overwrite. Sets
 * every keymap cell, NKRO, the layer / debounce / tuning configs, the RGB state and
 * zone table, the feature enables, and — clearing each behaviour / leader / macro
 * table first — the present slots, mirroring the firmware's `restore_blob`. The edits
 * are live (RAM) only; the caller persists them with `CONFIG.SAVE` (`client.configSave`).
 */
export async function writeFullConfig(client: ConfigClient, cfg: FullConfig): Promise<void> {
  for (let layer = 0; layer < cfg.keymap.length; layer += 1) {
    const rows = cfg.keymap[layer];
    for (let row = 0; row < rows.length; row += 1) {
      const cols = rows[row];
      for (let col = 0; col < cols.length; col += 1) {
        await client.setKeycode(layer, row, col, cols[col]);
      }
    }
  }

  await client.setKro(cfg.nkro);
  await client.setLayerConfig(cfg.layerConfig);
  await client.setDebounce(cfg.debounce);
  await client.setTuning(cfg.tuning);

  await client.rgbSetMode(cfg.rgb.mode);
  await client.rgbSetHsv(cfg.rgb.hue, cfg.rgb.sat, cfg.rgb.val);
  await client.rgbSetBrightness(cfg.rgb.brightness);
  await client.rgbSetEnabled(cfg.rgb.enabled);
  await client.rgbSetSpeed(cfg.rgb.speed);
  await client.rgbSetIndicators(cfg.rgb.indicators);
  await writeZones(client, cfg.zones);

  await client.socdClearAll();
  for (let i = 0; i < cfg.socd.length; i += 1) {
    const pair = cfg.socd[i];
    if (pair) await client.socdSet(i, pair);
  }

  await client.overrideClearAll();
  for (let i = 0; i < cfg.overrides.length; i += 1) {
    const override = cfg.overrides[i];
    if (override) await client.overrideSet(i, override);
  }

  await client.tapdanceClearAll();
  for (let i = 0; i < cfg.tapDance.length; i += 1) {
    const td = cfg.tapDance[i];
    if (td) await client.tapdanceSet(i, td);
  }

  await client.comboClearAll();
  for (let i = 0; i < cfg.combos.length; i += 1) {
    const combo = cfg.combos[i];
    if (combo) await client.comboSet(i, combo);
  }

  await client.leaderClearAll();
  for (let i = 0; i < cfg.leaders.length; i += 1) {
    const leader = cfg.leaders[i];
    if (leader) await client.leaderSet(i, leader);
  }

  await client.macroClearAll();
  for (let m = 0; m < cfg.macros.length; m += 1) {
    const steps = cfg.macros[m];
    for (let s = 0; s < steps.length; s += 1) {
      await client.macroSetStep(m, s, steps[s]);
    }
  }

  // Features always exist (no clear), so apply each record's enable directly. An
  // always-on feature is read back enabled, so this re-asserts enabled (accepted) and
  // never tries the disable the firmware would reject.
  for (const feature of cfg.features) {
    await client.setFeatureEnabled(feature.id, feature.enabled);
  }
}
