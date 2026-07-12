// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * RGB group (0x6x) wire helpers. The state and mode-list layouts mirror
 * `kcp.rs`'s `pack_rgb_state` / `CMD_RGB_LIST_MODES` byte-for-byte; the HSV→RGB
 * conversion is a direct port of the firmware's `hsv_to_rgb` so the GUI swatch
 * matches what the device emits.
 */

/**
 * Effect-mode ids (`rgb.rs` `MODE_*`), with GUI labels. The firmware reports the
 * authoritative id list over LIST_MODES; these labels name the known ones.
 */
export const RGB_MODE_LABELS: Record<number, string> = {
  0: 'Solid',
  1: 'Breathing',
  2: 'Rainbow',
  3: 'Gradient Up-Down',
  4: 'Gradient Left-Right',
  5: 'Cycle Up-Down',
  6: 'Cycle Left-Right',
  7: 'Band',
  8: 'Pinwheel',
  9: 'Raindrops',
  10: 'Solid Reactive',
  11: 'Reactive Wide',
  12: 'Cross',
  13: 'Splash',
  14: 'Reactive Rainbow',
  15: 'Band Value',
  16: 'Band Saturation',
  17: 'Pinwheel Band Value',
  18: 'Pinwheel Band Saturation',
  19: 'Spiral Band Value',
  20: 'Spiral Band Saturation',
  21: 'Cycle Out-In',
  22: 'Cycle Out-In Dual',
  23: 'Cycle Spiral',
  24: 'Rainbow Moving Chevron',
  25: 'Hue Breathing',
  26: 'Hue Pendulum',
  27: 'Hue Wave',
  28: 'Dual Beacon',
  29: 'Rainbow Beacon',
  30: 'Jellybean Raindrops',
  31: 'Pixel Rain',
  32: 'Pixel Flow',
  33: 'Starlight',
  34: 'Starlight Dual Hue',
  35: 'Solid Multisplash',
  36: 'Solid Reactive Multiwide',
  37: 'Multinexus',
  38: 'Alphas Mods',
  39: 'Rainbow Pinwheels',
  40: 'Pixel Fractal',
  41: 'Riverflow',
  42: 'Typing Heatmap',
  43: 'Digital Rain',
  44: 'Reactive Simple',
  45: 'Reactive Nexus',
  46: 'Solid Reactive Cross',
  47: 'Solid Reactive Multicross',
  48: 'Solid Splash',
  49: 'Starlight Dual Saturation',
};

/** Label for an effect-mode id (falls back to `Mode N` for unknown ids). */
export function rgbModeLabel(id: number): string {
  return RGB_MODE_LABELS[id] ?? `Mode ${id}`;
}

/**
 * A decoded RGB state (`pack_rgb_state`), payload-relative offsets:
 * `0` mode, `1` hue, `2` sat, `3` val, `4` brightness, `5` enabled (1/0),
 * `6..8` LED count (u16 LE), `8` animation speed, `9` indicators enabled (1/0).
 */
export interface RgbState {
  mode: number;
  hue: number;
  sat: number;
  val: number;
  brightness: number;
  enabled: boolean;
  ledCount: number;
  speed: number;
  /** Whether the firmware status-indicator overlay is drawn (persisted from schema v7). */
  indicators: boolean;
}

/** Parse a GET_STATE reply payload into an {@link RgbState}. */
export function parseRgbState(payload: Uint8Array): RgbState {
  return {
    mode: payload[0],
    hue: payload[1],
    sat: payload[2],
    val: payload[3],
    brightness: payload[4],
    enabled: payload[5] !== 0,
    ledCount: (payload[6] | (payload[7] << 8)) & 0xffff,
    speed: payload[8],
    indicators: payload[9] !== 0,
  };
}

/** Zone flag bit 0 — the zone is lit (clear blanks its LED range). */
export const ZONE_FLAG_ENABLED = 0x01;
/** Zone flag bit 1 — the zone mirrors the base effect (clear runs its own effect). */
export const ZONE_FLAG_LINKED = 0x02;
/** `rgb.rs` `ZONE_SYNC_NONE` — a zone's {@link ZoneState.syncTo} when it is not synced. */
export const ZONE_SYNC_NONE = 0xff;
/** Highest addressable chain index + 1 (`rgb.rs` `LED_COUNT`) — the resize upper bound. */
export const LED_COUNT = 105;

/** GUI labels for the v1 zones (`rgb.rs` `DEFAULT_ZONES`). */
export const ZONE_LABELS: Record<number, string> = {
  0: 'Keys',
  1: 'Right',
  2: 'Left',
};

/** Label for a zone id (falls back to `Zone N` for unknown ids). */
export function zoneLabel(id: number): string {
  return ZONE_LABELS[id] ?? `Zone ${id}`;
}

/** The zone-table summary (`GET_ZONES`): how many zones the GUI lists, and the cap. */
export interface ZonesInfo {
  zoneCount: number;
  zoneCap: number;
}

/** Parse a GET_ZONES reply payload `[zone_count, zone_cap]` into a {@link ZonesInfo}. */
export function parseZones(payload: Uint8Array): ZonesInfo {
  return { zoneCount: payload[0], zoneCap: payload[1] };
}

/**
 * One zone's state (`pack_zone`), payload-relative offsets: `0` id, `1` flags
 * (bit0 ENABLED, bit1 LINKED), `2` mode, `3` hue, `4` sat, `5` val, `6` brightness,
 * `7` speed, `8..10` range start (u16 LE), `10..12` range LED count (u16 LE), `12`
 * sync source on the wire (`0` = not synced, else zone id + 1; {@link parseZone}
 * decodes it to {@link ZoneState.syncTo}). A linked zone mirrors the base effect in
 * its range; an independent zone runs its own effect from these params; a synced zone
 * mirrors {@link ZoneState.syncTo}'s effect in its own range; a disabled (not ENABLED)
 * zone is blanked. Carried by GET_ZONE / SET_ZONE (0x69/0x6A) — SET_ZONE round-trips
 * bytes `1..8`; the sync byte is set separately by SET_ZONE_SYNC (0x6D).
 */
export interface ZoneState {
  id: number;
  flags: number;
  mode: number;
  hue: number;
  sat: number;
  val: number;
  brightness: number;
  speed: number;
  start: number;
  count: number;
  /** Sync source zone id, or {@link ZONE_SYNC_NONE} when the zone shows its own effect. */
  syncTo: number;
}

/** Parse a GET_ZONE reply payload into a {@link ZoneState}. */
export function parseZone(payload: Uint8Array): ZoneState {
  return {
    id: payload[0],
    flags: payload[1],
    mode: payload[2],
    hue: payload[3],
    sat: payload[4],
    val: payload[5],
    brightness: payload[6],
    speed: payload[7],
    start: (payload[8] | (payload[9] << 8)) & 0xffff,
    count: (payload[10] | (payload[11] << 8)) & 0xffff,
    // The wire byte is biased (firmware `sync_to_wire`): 0 = not synced, else zone
    // id + 1. The bias lets a zero — an older firmware that zero-fills its GET_ZONE
    // reply — decode as not-synced rather than "synced to zone 0".
    syncTo: payload[12] === 0 ? ZONE_SYNC_NONE : payload[12] - 1,
  };
}

/** Serialise a {@link ZoneState} into the 8-byte SET_ZONE request payload. */
export function packZoneArgs(z: ZoneState): number[] {
  return [
    z.id & 0xff,
    z.flags & 0xff,
    z.mode & 0xff,
    z.hue & 0xff,
    z.sat & 0xff,
    z.val & 0xff,
    z.brightness & 0xff,
    z.speed & 0xff,
  ];
}

/** Whether a zone is lit (the ENABLED flag bit). */
export function zoneEnabled(z: ZoneState): boolean {
  return (z.flags & ZONE_FLAG_ENABLED) !== 0;
}

/** Whether a zone mirrors the base effect (the LINKED flag bit). */
export function zoneLinked(z: ZoneState): boolean {
  return (z.flags & ZONE_FLAG_LINKED) !== 0;
}

/** Whether a zone mirrors another zone's effect (its {@link ZoneState.syncTo} is set). */
export function zoneSynced(z: ZoneState): boolean {
  return z.syncTo !== ZONE_SYNC_NONE;
}

/**
 * Whether placing zone `id` at `start..start+count` would overlap another enabled,
 * non-empty zone in `zones` — the client-side mirror of the firmware
 * `zone_range_overlaps`, so the resize control can reject an overlap before the
 * round-trip (the firmware re-checks authoritatively, and across the full zone table
 * rather than only the listed zones). An empty proposed range overlaps nothing.
 */
export function zoneRangeOverlaps(
  zones: ZoneState[],
  id: number,
  start: number,
  count: number,
): boolean {
  if (count <= 0) {
    return false;
  }
  return zones.some(
    (z) =>
      z.id !== id &&
      (z.flags & ZONE_FLAG_ENABLED) !== 0 &&
      z.count > 0 &&
      start < z.start + z.count &&
      z.start < start + count,
  );
}

/** Set or clear `bit` in a zone `flags` byte, preserving the other bits. */
export function withZoneFlag(flags: number, bit: number, on: boolean): number {
  return (on ? flags | bit : flags & ~bit) & 0xff;
}

/** One page of a LIST_MODES reply: the total mode count and this page's ids. */
export interface ModeListPage {
  /** Total number of effect modes across all pages (`rgb.rs` `MODE_COUNT`). */
  total: number;
  /** The mode ids carried in this page (`rgb.rs` `MODE_IDS[start..]`). */
  ids: number[];
}

/**
 * Parse one LIST_MODES reply page `[total, page_len, id_start, …]`. The firmware pages the
 * mode list by start offset once the set outgrows a single 29-byte reply, so the caller
 * (`client.rgbListModes`) requests successive pages and concatenates {@link
 * ModeListPage.ids} until it has `total` ids.
 */
export function parseModeList(payload: Uint8Array): ModeListPage {
  const total = payload[0];
  const pageLen = payload[1];
  const ids: number[] = [];
  for (let i = 0; i < pageLen; i += 1) {
    ids.push(payload[2 + i]);
  }
  return { total, ids };
}

/** An 8-bit-per-channel RGB colour. */
export interface Rgb {
  r: number;
  g: number;
  b: number;
}

/**
 * Convert HSV (each component 0..=255, hue wrapping the wheel) to {@link Rgb},
 * an integer-only six-sextant interpolation — a direct port of the firmware's
 * `rgb.rs` `hsv_to_rgb`, so a preview swatch matches the device output. With
 * `s == 0` the result is the grey `(v, v, v)`.
 */
export function hsvToRgb(h: number, s: number, v: number): Rgb {
  if (s === 0) {
    return { r: v, g: v, b: v };
  }
  const region = Math.floor(h / 43);
  const remainder = (h - region * 43) * 6;

  const p = Math.floor((v * (255 - s)) / 255);
  const q = Math.floor((v * (255 - Math.floor((s * remainder) / 255))) / 255);
  const t = Math.floor((v * (255 - Math.floor((s * (255 - remainder)) / 255))) / 255);

  switch (region) {
    case 0:
      return { r: v, g: t, b: p };
    case 1:
      return { r: q, g: v, b: p };
    case 2:
      return { r: p, g: v, b: t };
    case 3:
      return { r: p, g: q, b: v };
    case 4:
      return { r: t, g: p, b: v };
    default:
      return { r: v, g: p, b: q };
  }
}

/** Format an {@link Rgb} as a CSS `rgb(...)` string, for the swatch. */
export function rgbToCss({ r, g, b }: Rgb): string {
  return `rgb(${r}, ${g}, ${b})`;
}
