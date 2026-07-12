// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Test fixture: a faithful re-implementation of the firmware's reply packing
 * from `firmware/src/kcp.rs`, used by the codec/parser/transport tests.
 *
 * Mirroring the firmware's own packing (rather than restating the parser's
 * expectations) is what makes the suite a real byte-for-byte conformance check:
 * the parsers must read back exactly what `pack_device_info`, the capabilities
 * `to_le_bytes`, and `PROTOCOL_VERSION` write. Nothing in the app imports this
 * module, so it is never bundled.
 */
import { Cmd, Group, REPLY_FLAG, REPLY_PAYLOAD_LEN, Status } from './protocol';

/** `PROTOCOL_VERSION: [u8; 2]` in `kcp.rs`. */
export const PROTOCOL_VERSION: readonly [number, number] = [0, 2];
/** `FIRMWARE_VERSION: [u8; 3]` in `kcp.rs`. */
export const FIRMWARE_VERSION: readonly [number, number, number] = [0, 1, 0];
/** `CHIP_ID: &[u8; 8]` in `kcp.rs`. */
export const CHIP_ID = 'WB32FQ95';
/** `CONN_USB: u8 = 0` in `kcp.rs`. */
export const CONN_USB = 0;
/** `NUM_ROWS` (matrix.rs). */
export const NUM_ROWS = 6;
/** `NUM_COLS` (matrix.rs). */
export const NUM_COLS = 15;
/** `LAYERS` (keymap.rs). */
export const LAYERS = 16;
/**
 * `config::SCHEMA_VERSION: u16 = 10` (config.rs) — the on-flash config schema
 * version `pack_device_info` reports and `config::save` stamps into the blob.
 * Version 10 replaced the standalone autocorrect flag with the registry-wide
 * feature-enable bitmap; version 9 grew the keymap from eight to sixteen layers (the
 * keymap blob doubled); version 8 appended the autocorrect enable flag; version 7 added
 * the RGB zone table and persisted the status-indicator overlay flag; version 6 added the
 * per-combo flags byte and the layer-config block (the default `DF` layer and the
 * tri-layer rule).
 */
export const SCHEMA_VERSION = 10;

/**
 * `CAPABILITIES` from `kcp.rs`, mirrored byte-for-byte so the parser is checked
 * against the firmware's own mask:
 * `(1 << INFO) | (1 << KEYMAP) | (1 << TELEMETRY) | (1 << HID_KRO) | (1 << CONFIG)
 *  | (1 << MACRO) | (1 << RGB) | (1 << BEHAVIOR) | (1 << WIRELESS) | (1 << TEXT)
 *  | (1 << UNICODE) | (1 << FEATURES) | (1 << SYSTEM)` — which the firmware notes
 * equals 0xA7FF.
 * ({@link fakeFirmwareHandle} below only models the subset of groups the client wrappers
 * exercise; the advertised mask still mirrors the firmware exactly.)
 */
export const CAPABILITIES =
  ((1 << Group.Info) |
    (1 << Group.Keymap) |
    (1 << Group.Telemetry) |
    (1 << Group.HidKro) |
    (1 << Group.Config) |
    (1 << Group.Macro) |
    (1 << Group.Rgb) |
    (1 << Group.Behavior) |
    (1 << Group.Wireless) |
    (1 << Group.Text) |
    (1 << Group.Unicode) |
    (1 << Group.Features) |
    (1 << Group.System)) >>>
  0;

/** `SCAN_RATE_HZ: u16 = 1000` in `kcp.rs`. */
export const SCAN_RATE_HZ = 1000;
/** `TELEMETRY_UNAVAILABLE: u8 = 0xFF` in `kcp.rs`. */
export const TELEMETRY_UNAVAILABLE = 0xff;
/** `rgb::MODE_COUNT` (rgb.rs). */
export const MODE_COUNT = 50;
/**
 * `rgb::MODE_IDS` = `MODE_SOLID..MODE_STARLIGHT_DUAL_SAT` (rgb.rs). The effect mode ids are
 * the contiguous registry indices `0..MODE_COUNT`, so this mirrors the firmware's hand-written
 * id list exactly.
 */
export const MODE_IDS: readonly number[] = Array.from({ length: MODE_COUNT }, (_, i) => i);
/** `rgb::LED_COUNT` (rgb.rs). */
export const LED_COUNT = 105;
/** `rgb::ZONE_COUNT` (rgb.rs) — the v1 zones the GUI lists (Keys, Right, Left). */
export const ZONE_COUNT = 3;
/** `rgb::ZONE_CAP` (rgb.rs) — the zone-table capacity. */
export const ZONE_CAP = 4;
/** `rgb::ZONE_FLAG_ENABLED` (rgb.rs). */
export const ZONE_FLAG_ENABLED = 0x01;
/** `rgb::ZONE_FLAG_LINKED` (rgb.rs). */
export const ZONE_FLAG_LINKED = 0x02;
/** `rgb::ZONE_SYNC_NONE` (rgb.rs) — a zone's `syncTo` when it is not synced. */
export const ZONE_SYNC_NONE = 0xff;

/**
 * The feature registry (`features::FEATURES` order), mirroring each feature's stable
 * `FeatureId` discriminant, GUI `name()` and `FEATURE_ALWAYS_ON` flag. The order and ids
 * are exactly what GET_FEATURES enumerates; `alwaysOn` is the structural core (SOCD, key
 * overrides, the timed engine) that SET_FEATURE_ENABLED refuses to switch off. Autocorrect
 * shares its enable with the TEXT group (folded into the central bitmap), so both round-trip
 * the same {@link FakeDevice.featuresEnabled} bit.
 */
export const FEATURE_DEFS: readonly { id: number; name: string; alwaysOn: boolean }[] = [
  { id: 0, name: 'SOCD Cleanup', alwaysOn: true },
  { id: 1, name: 'Key Overrides', alwaysOn: true },
  { id: 2, name: 'Timed Engine', alwaysOn: true },
  { id: 8, name: 'Unicode', alwaysOn: false },
  { id: 5, name: 'Repeat Key', alwaysOn: false },
  { id: 3, name: 'Caps Word', alwaysOn: false },
  { id: 4, name: 'Key Lock', alwaysOn: false },
  { id: 6, name: 'One-Shot Mod', alwaysOn: false },
  { id: 7, name: 'Autocorrect', alwaysOn: false },
  // @scaffold:fixture-features — `just new-feature <Name>` appends each new feature's
  // `{ id, name, alwaysOn: false }` row here (the registry END), so the simulated
  // GET_FEATURES enumeration stays in firmware `FEATURES` order across repeated generations.
] as const;

/** `FeatureId::Autocorrect` (`features/mod.rs`): the bit the TEXT group shares. */
export const FEATURE_ID_AUTOCORRECT = 7;

/** Allocate a zeroed reply payload (29 bytes), as `handle` hands the group. */
function replyPayload(): Uint8Array {
  return new Uint8Array(REPLY_PAYLOAD_LEN);
}

/** `info_dispatch(CMD_GET_VERSION)`: `out[..2] = PROTOCOL_VERSION`. */
export function packProtocolVersion(): Uint8Array {
  const out = replyPayload();
  out[0] = PROTOCOL_VERSION[0];
  out[1] = PROTOCOL_VERSION[1];
  return out;
}

/** `info_dispatch(CMD_GET_CAPABILITIES)`: `out[..4] = CAPABILITIES.to_le_bytes()`. */
export function packCapabilities(mask = CAPABILITIES): Uint8Array {
  const out = replyPayload();
  out[0] = mask & 0xff;
  out[1] = (mask >>> 8) & 0xff;
  out[2] = (mask >>> 16) & 0xff;
  out[3] = (mask >>> 24) & 0xff;
  return out;
}

/**
 * `pack_device_info`: firmware version, 8-byte chip id, rows, cols, layers, the
 * transport byte and the config schema version (u16 LE), at the offsets
 * documented in `kcp.rs`.
 */
export function packDeviceInfo(connection = CONN_USB, schemaVersion = SCHEMA_VERSION): Uint8Array {
  const out = replyPayload();
  out[0] = FIRMWARE_VERSION[0];
  out[1] = FIRMWARE_VERSION[1];
  out[2] = FIRMWARE_VERSION[2];
  for (let i = 0; i < 8; i += 1) {
    out[3 + i] = CHIP_ID.charCodeAt(i);
  }
  out[11] = NUM_ROWS;
  out[12] = NUM_COLS;
  out[13] = LAYERS;
  out[14] = connection;
  out[15] = schemaVersion & 0xff;
  out[16] = (schemaVersion >> 8) & 0xff;
  return out;
}

// === KEYMAP group ==========================================================

/** `keymap_dispatch(CMD_GET_KEYCODE)`: `out[..2] = kc.to_le_bytes()`. */
export function packKeycodeReply(keycode: number): Uint8Array {
  const out = replyPayload();
  out[0] = keycode & 0xff;
  out[1] = (keycode >> 8) & 0xff;
  return out;
}

/** `keymap_dispatch(CMD_GET_LAYER_COUNT)`: `out[0] = LAYERS`. */
export function packLayerCount(layers = LAYERS): Uint8Array {
  const out = replyPayload();
  out[0] = layers;
  return out;
}

/** The layer config `keymap.rs` holds: the default `DF` layer and the tri-layer rule. */
export interface LayerConfigSample {
  defaultLayer: number;
  triEnabled: boolean;
  triL1: number;
  triL2: number;
  triL3: number;
}

/** The power-on layer-config defaults (base layer 0, tri-layer off). */
export const DEFAULT_LAYER_CONFIG: LayerConfigSample = {
  defaultLayer: 0,
  triEnabled: false,
  triL1: 0,
  triL2: 0,
  triL3: 0,
};

/** `keymap_dispatch(CMD_GET_LAYER_CONFIG)`: `[default_layer, tri_enabled, l1, l2, l3]`. */
export function packLayerConfig(c: LayerConfigSample): Uint8Array {
  const out = replyPayload();
  out[0] = c.defaultLayer & 0xff;
  out[1] = c.triEnabled ? 1 : 0;
  out[2] = c.triL1 & 0xff;
  out[3] = c.triL2 & 0xff;
  out[4] = c.triL3 & 0xff;
  return out;
}

// === TELEMETRY group =======================================================

/** A telemetry snapshot, the fields `pack_telemetry` samples from the device. */
export interface TelemetrySample {
  uptimeMs: number;
  scanCount: number;
  reportCount: number;
  activeLayers: number;
  scanRateHz: number;
  lastProcUs: number;
  battery: number;
  rssi: number;
  connection: number;
}

/** A representative on-USB snapshot (battery defaults to 100, RSSI unavailable). */
export const DEFAULT_TELEMETRY: TelemetrySample = {
  uptimeMs: 1_234_567,
  scanCount: 1_234_000,
  reportCount: 4_096,
  activeLayers: 0b01,
  scanRateHz: SCAN_RATE_HZ,
  lastProcUs: 142,
  battery: 100,
  rssi: TELEMETRY_UNAVAILABLE,
  connection: CONN_USB,
};

/** `pack_telemetry`: every multi-byte field little-endian, at the exact offsets. */
export function packTelemetry(t: TelemetrySample = DEFAULT_TELEMETRY): Uint8Array {
  const out = replyPayload();
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setUint32(0, t.uptimeMs >>> 0, true);
  view.setUint32(4, t.scanCount >>> 0, true);
  view.setUint32(8, t.reportCount >>> 0, true);
  view.setUint16(12, t.activeLayers & 0xffff, true);
  view.setUint16(14, t.scanRateHz & 0xffff, true);
  view.setUint32(16, t.lastProcUs >>> 0, true);
  out[20] = t.battery & 0xff;
  out[21] = t.rssi & 0xff;
  out[22] = t.connection & 0xff;
  return out;
}

// === RGB group =============================================================

/** The live RGB state `pack_rgb_state` samples. */
export interface RgbStateSample {
  mode: number;
  hue: number;
  sat: number;
  val: number;
  brightness: number;
  enabled: boolean;
  ledCount: number;
  speed: number;
  indicators: boolean;
}

/** The power-on RGB defaults (`rgb.rs` static initialisers). */
export const DEFAULT_RGB_STATE: RgbStateSample = {
  mode: 2, // MODE_RAINBOW
  hue: 0,
  sat: 255,
  val: 255,
  brightness: 128,
  enabled: true,
  ledCount: LED_COUNT,
  speed: 128,
  indicators: true,
};

/**
 * `pack_rgb_state`: mode, h, s, v, brightness, enabled, LED count u16 LE, the
 * animation speed, then the status-indicator flag.
 */
export function packRgbState(s: RgbStateSample = DEFAULT_RGB_STATE): Uint8Array {
  const out = replyPayload();
  out[0] = s.mode & 0xff;
  out[1] = s.hue & 0xff;
  out[2] = s.sat & 0xff;
  out[3] = s.val & 0xff;
  out[4] = s.brightness & 0xff;
  out[5] = s.enabled ? 1 : 0;
  out[6] = s.ledCount & 0xff;
  out[7] = (s.ledCount >> 8) & 0xff;
  out[8] = s.speed & 0xff;
  out[9] = s.indicators ? 1 : 0;
  return out;
}

/** One zone's state (`rgb.rs` `Zone`), as the zone table `pack_zone` samples. */
export interface ZoneSample {
  flags: number;
  mode: number;
  hue: number;
  sat: number;
  val: number;
  brightness: number;
  speed: number;
  start: number;
  count: number;
  syncTo: number;
}

const ZONE_FLAGS_DEFAULT = ZONE_FLAG_ENABLED | ZONE_FLAG_LINKED;

/** A default zone over `start..start+count`: enabled + linked, power-on colour, no sync. */
function defaultZone(start: number, count: number): ZoneSample {
  return {
    flags: ZONE_FLAGS_DEFAULT,
    mode: 2, // MODE_RAINBOW (the base-effect default the zone mirrors when linked)
    hue: 0,
    sat: 255,
    val: 255,
    brightness: 128,
    speed: 128,
    start,
    count,
    syncTo: ZONE_SYNC_NONE,
  };
}

/** The power-on zone table (`rgb.rs` `DEFAULT_ZONES`): Keys, Right, Left + a spare. */
export const DEFAULT_ZONES: ZoneSample[] = [
  defaultZone(0, 83), // id 0 — Keys
  defaultZone(83, 11), // id 1 — Right side strip
  defaultZone(94, 11), // id 2 — Left side strip
  defaultZone(0, 0), // id 3 — reserved
];

/** `rgb_dispatch(CMD_RGB_GET_ZONES)`: `[zone_count, zone_cap]`. */
export function packZones(): Uint8Array {
  const out = replyPayload();
  out[0] = ZONE_COUNT;
  out[1] = ZONE_CAP;
  return out;
}

/** `pack_zone`: id, flags, mode, h, s, v, brightness, speed, start(2 LE), count(2 LE),
 * and the biased sync byte (`sync_to_wire`: 0 = not synced, else syncTo + 1). */
export function packZone(id: number, z: ZoneSample): Uint8Array {
  const out = replyPayload();
  out[0] = id & 0xff;
  out[1] = z.flags & 0xff;
  out[2] = z.mode & 0xff;
  out[3] = z.hue & 0xff;
  out[4] = z.sat & 0xff;
  out[5] = z.val & 0xff;
  out[6] = z.brightness & 0xff;
  out[7] = z.speed & 0xff;
  out[8] = z.start & 0xff;
  out[9] = (z.start >> 8) & 0xff;
  out[10] = z.count & 0xff;
  out[11] = (z.count >> 8) & 0xff;
  // Bias the sync byte like the firmware `sync_to_wire`: 0 = not synced, else id + 1,
  // so a zero-filled reply reads as not-synced rather than "synced to zone 0".
  out[12] = z.syncTo === ZONE_SYNC_NONE ? 0 : (z.syncTo + 1) & 0xff;
  return out;
}

/** `kcp.rs` `RGB_MODE_PAGE`: mode ids carried per LIST_MODES reply page. */
export const RGB_MODE_PAGE = REPLY_PAYLOAD_LEN - 2;

/**
 * `rgb_dispatch(CMD_RGB_LIST_MODES)`: one reply page `[total, page_len, id_start, …]`
 * paging {@link MODE_IDS} from `start` (the firmware caps each page at {@link
 * RGB_MODE_PAGE} ids and the host requests successive pages).
 */
export function packRgbModeList(start = 0): Uint8Array {
  const out = replyPayload();
  const from = Math.min(start, MODE_IDS.length);
  const n = Math.min(MODE_IDS.length - from, RGB_MODE_PAGE);
  out[0] = MODE_COUNT;
  out[1] = n;
  for (let i = 0; i < n; i += 1) {
    out[2 + i] = MODE_IDS[from + i];
  }
  return out;
}

// === CONFIG group ==========================================================

/** `flash::CONFIG_REGION.start` (flash.rs): the last `CONFIG_PAGES` (18) flash pages. */
export const CONFIG_REGION_BASE = 0x0801_ee00;
/** `CONFIG_REGION.end - CONFIG_REGION.start` = 4608 bytes (the last 18 pages, `0x1200`). */
export const CONFIG_REGION_SIZE = 0x1200;

/** The storage descriptor `pack_storage_info` samples from `config::storage_info`. */
export interface StorageInfoSample {
  base: number;
  size: number;
  version: number;
  valid: boolean;
}

/** A fresh device: the region described, with no valid blob stored yet. */
export const DEFAULT_STORAGE_INFO: StorageInfoSample = {
  base: CONFIG_REGION_BASE,
  size: CONFIG_REGION_SIZE,
  version: 0,
  valid: false,
};

/** `pack_storage_info`: base u32, size u32, version u16, valid u8 — all LE. */
export function packStorageInfo(s: StorageInfoSample = DEFAULT_STORAGE_INFO): Uint8Array {
  const out = replyPayload();
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setUint32(0, s.base >>> 0, true);
  view.setUint32(4, s.size >>> 0, true);
  view.setUint16(8, s.version & 0xffff, true);
  out[10] = s.valid ? 1 : 0;
  return out;
}

/** `matrix::DebounceAlgorithm` codes (matrix.rs): symmetric-defer, eager-on-press. */
export const DEBOUNCE_SYMMETRIC = 0;
export const DEBOUNCE_EAGER = 1;
/** `matrix::DEFAULT_DEBOUNCE_INTERVAL` (matrix.rs). */
export const DEFAULT_DEBOUNCE_INTERVAL = 5;

/** The matrix debounce config `pack`/`set` exchange (`matrix::{algorithm, interval}`). */
export interface DebounceSample {
  algorithm: number;
  interval: number;
}

/** The power-on debounce defaults (`matrix.rs` static initialisers). */
export const DEFAULT_DEBOUNCE: DebounceSample = {
  algorithm: DEBOUNCE_SYMMETRIC,
  interval: DEFAULT_DEBOUNCE_INTERVAL,
};

/** `config_dispatch(CMD_CONFIG_GET_DEBOUNCE)`: `[algorithm, interval]`. */
export function packDebounce(d: DebounceSample = DEFAULT_DEBOUNCE): Uint8Array {
  const out = replyPayload();
  out[0] = d.algorithm & 0xff;
  out[1] = d.interval & 0xff;
  return out;
}

/** `timed::DEFAULT_AUTO_SHIFT_TIMEOUT_MS` (timed.rs). */
export const DEFAULT_AUTO_SHIFT_TIMEOUT_MS = 175;
/** `timed::DEFAULT_LEADER_TIMEOUT_MS` (timed.rs). */
export const DEFAULT_LEADER_TIMEOUT_MS = 300;
/** `timed::DEFAULT_TAP_HOLD_TERM_MS` (timed.rs). */
export const DEFAULT_TAP_HOLD_TERM_MS = 200;
/** `timed::DEFAULT_QUICK_TAP_TERM_MS` (timed.rs). */
export const DEFAULT_QUICK_TAP_TERM_MS = 200;

/** The runtime tunables `CONFIG.GET_TUNING` / `SET_TUNING` exchange (`crate::timed`). */
export interface TuningSample {
  autoShiftEnabled: boolean;
  autoShiftTimeoutMs: number;
  leaderTimeoutMs: number;
  tapHoldTermMs: number;
  permissiveHold: boolean;
  holdOnOtherKeyPress: boolean;
  retroTapping: boolean;
  chordalHold: boolean;
  quickTapTermMs: number;
}

/** The power-on tunable defaults (auto-shift off, default timeouts, default tap-hold tuning). */
export const DEFAULT_TUNING: TuningSample = {
  autoShiftEnabled: false,
  autoShiftTimeoutMs: DEFAULT_AUTO_SHIFT_TIMEOUT_MS,
  leaderTimeoutMs: DEFAULT_LEADER_TIMEOUT_MS,
  tapHoldTermMs: DEFAULT_TAP_HOLD_TERM_MS,
  permissiveHold: false,
  holdOnOtherKeyPress: false,
  retroTapping: false,
  chordalHold: false,
  quickTapTermMs: DEFAULT_QUICK_TAP_TERM_MS,
};

/**
 * `config_dispatch(CMD_CONFIG_GET_TUNING)`: `[as_on, as_timeout(2 LE),
 * leader_timeout(2 LE), tap_hold_term(2 LE), tap_hold_flags(1), quick_tap_term(2 LE)]`.
 * The flags byte mirrors `TapHoldTuning::flags_byte` (bit 0 permissive, bit 1
 * hold-on-other, bit 2 retro, bit 3 chordal).
 */
export function packTuning(t: TuningSample = DEFAULT_TUNING): Uint8Array {
  const out = replyPayload();
  out[0] = t.autoShiftEnabled ? 1 : 0;
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setUint16(1, t.autoShiftTimeoutMs & 0xffff, true);
  view.setUint16(3, t.leaderTimeoutMs & 0xffff, true);
  view.setUint16(5, t.tapHoldTermMs & 0xffff, true);
  out[7] =
    (t.permissiveHold ? 1 : 0) |
    (t.holdOnOtherKeyPress ? 2 : 0) |
    (t.retroTapping ? 4 : 0) |
    (t.chordalHold ? 8 : 0);
  view.setUint16(8, t.quickTapTermMs & 0xffff, true);
  return out;
}

// === HID_KRO group =========================================================

/** `hid_kro_dispatch(CMD_GET_KRO)`: `out[0] = nkro_enabled`. */
export function packKro(nkroEnabled: boolean): Uint8Array {
  const out = replyPayload();
  out[0] = nkroEnabled ? 1 : 0;
  return out;
}

// === MACRO group ===========================================================

/** `timed::MAX_MACRO` (timed.rs). */
export const MAX_MACRO = 4;
/** `timed::MAX_MACRO_STEPS` (timed.rs). */
export const MAX_MACRO_STEPS = 32;

/** One macro step (`timed::MacroStep`); keycode is raw u16. */
export interface MacroStepSample {
  kc: number;
  down: boolean;
  delayMs: number;
}

/** One macro slot (`timed::MacroCfg`): a fixed step array and an active length. */
export interface MacroSlotSample {
  steps: MacroStepSample[];
  len: number;
}

/** An empty macro step (`MacroStep::EMPTY`). */
function emptyMacroStep(): MacroStepSample {
  return { kc: 0, down: false, delayMs: 0 };
}

/** A fresh, empty macro slot (`MacroCfg::EMPTY`). */
function emptyMacroSlot(): MacroSlotSample {
  return { steps: Array.from({ length: MAX_MACRO_STEPS }, emptyMacroStep), len: 0 };
}

/** `macro_dispatch(CMD_MACRO_INFO)`: `[MAX_MACRO, MAX_MACRO_STEPS, used(4 LE)]`. */
export function packMacroInfo(macros: MacroSlotSample[]): Uint8Array {
  const out = replyPayload();
  out[0] = MAX_MACRO;
  out[1] = MAX_MACRO_STEPS;
  let used = 0;
  macros.forEach((m, i) => {
    if (m.len > 0) used |= 1 << i;
  });
  const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
  view.setUint32(2, used >>> 0, true);
  return out;
}

/**
 * `macro_dispatch(CMD_MACRO_GET_STEP)`: `[present, kc_lo, kc_hi, down, delay_lo,
 * delay_hi, len]` where `present = step < len`.
 */
export function packMacroStep(slot: MacroSlotSample, step: number): Uint8Array {
  const out = replyPayload();
  const ev = slot.steps[step];
  out[0] = step < slot.len ? 1 : 0;
  out[1] = ev.kc & 0xff;
  out[2] = (ev.kc >> 8) & 0xff;
  out[3] = ev.down ? 1 : 0;
  out[4] = ev.delayMs & 0xff;
  out[5] = (ev.delayMs >> 8) & 0xff;
  out[6] = slot.len;
  return out;
}

// === BEHAVIOR group ========================================================

/** `behavior::MAX_SOCD` (behavior.rs). */
export const MAX_SOCD = 8;
/** `behavior::MAX_OVERRIDES` (behavior.rs). */
export const MAX_OVERRIDES = 16;
/** `behavior::CLEAR_ALL` (behavior.rs) — the clear-whole-table sentinel index. */
const CLEAR_ALL = 0xff;

/** A configured SOCD pair (`behavior::SocdPair`); keycodes are raw u16. */
export interface SocdPairSample {
  a: number;
  b: number;
  mode: number;
}

/** A configured key override (`behavior::KeyOverride`). */
export interface KeyOverrideSample {
  trigger: number;
  triggerMods: number;
  replacement: number;
  replacementMods: number;
  layerMask: number;
  enabled: boolean;
}

/** `pack_socd_pair`: `present` then `[a_lo, a_hi, b_lo, b_hi, mode]`; empty = 0. */
export function packSocdPair(pair: SocdPairSample | null): Uint8Array {
  const out = replyPayload();
  if (pair) {
    out[0] = 1;
    out[1] = pair.a & 0xff;
    out[2] = (pair.a >> 8) & 0xff;
    out[3] = pair.b & 0xff;
    out[4] = (pair.b >> 8) & 0xff;
    out[5] = pair.mode & 0xff;
  }
  return out;
}

/**
 * `pack_override`: `present` then `[trig_lo, trig_hi, trig_mods, repl_lo, repl_hi,
 * repl_mods, layer_lo, layer_hi, enabled]`; an empty slot writes `present = 0`.
 */
export function packOverride(ov: KeyOverrideSample | null): Uint8Array {
  const out = replyPayload();
  if (ov) {
    out[0] = 1;
    out[1] = ov.trigger & 0xff;
    out[2] = (ov.trigger >> 8) & 0xff;
    out[3] = ov.triggerMods & 0xff;
    out[4] = ov.replacement & 0xff;
    out[5] = (ov.replacement >> 8) & 0xff;
    out[6] = ov.replacementMods & 0xff;
    out[7] = ov.layerMask & 0xff;
    out[8] = (ov.layerMask >> 8) & 0xff;
    out[9] = ov.enabled ? 1 : 0;
  }
  return out;
}

/** `behavior_dispatch(CMD_BEHAVIOR_INFO)`: `[MAX_SOCD, MAX_OVERRIDES]`. */
export function packBehaviorInfo(): Uint8Array {
  const out = replyPayload();
  out[0] = MAX_SOCD;
  out[1] = MAX_OVERRIDES;
  return out;
}

/** `timed::MAX_TAP_DANCE` (timed.rs). */
export const MAX_TAP_DANCE = 8;
/** `timed::MAX_COMBO` (timed.rs). */
export const MAX_COMBO = 8;
/** `timed::MAX_COMBO_KEYS` (timed.rs). */
export const MAX_COMBO_KEYS = 4;
/** `timed::MIN_COMBO_KEYS` (timed.rs). */
export const MIN_COMBO_KEYS = 2;
/** `timed::MAX_LEADER` (timed.rs). */
export const MAX_LEADER = 8;
/** `timed::MAX_LEADER_SEQ` (timed.rs). */
export const MAX_LEADER_SEQ = 5;

/** A configured tap-dance entry (`timed::TapDanceCfg`); keycodes are raw u16. */
export interface TapDanceSample {
  tap: number;
  hold: number;
  double: number;
  termMs: number;
}

/** Per-combo flag bits (`timed::ComboCfg` `FLAG_*`). */
export const COMBO_FLAG_MUST_HOLD = 1 << 0;
export const COMBO_FLAG_MUST_TAP = 1 << 1;
export const COMBO_FLAG_IN_ORDER = 1 << 2;
/** Every defined combo flag bit (`ComboCfg::FLAG_MASK`). */
const COMBO_FLAG_MASK = COMBO_FLAG_MUST_HOLD | COMBO_FLAG_MUST_TAP | COMBO_FLAG_IN_ORDER;

/** A configured combo (`timed::ComboCfg`); `keys` are the four raw u16 slots. */
export interface ComboSample {
  keys: [number, number, number, number];
  len: number;
  action: number;
  termMs: number;
  flags: number;
}

/** `pack_tapdance`: `present` then `[tap, hold, double, term]` (each u16 LE). */
export function packTapdance(td: TapDanceSample | null): Uint8Array {
  const out = replyPayload();
  if (td) {
    out[0] = 1;
    const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
    view.setUint16(1, td.tap & 0xffff, true);
    view.setUint16(3, td.hold & 0xffff, true);
    view.setUint16(5, td.double & 0xffff, true);
    view.setUint16(7, td.termMs & 0xffff, true);
  }
  return out;
}

/**
 * `pack_combo`: `present`, `len`, the four member keycodes (u16 LE; unused slots
 * zero), the action keycode, the term and the per-combo flags byte. An empty slot
 * writes `present = 0`.
 */
export function packCombo(combo: ComboSample | null): Uint8Array {
  const out = replyPayload();
  if (combo) {
    out[0] = 1;
    out[1] = combo.len;
    const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
    combo.keys.forEach((k, i) => view.setUint16(2 + i * 2, k & 0xffff, true));
    view.setUint16(10, combo.action & 0xffff, true);
    view.setUint16(12, combo.termMs & 0xffff, true);
    out[14] = combo.flags & 0xff;
  }
  return out;
}

/**
 * `behavior_dispatch(CMD_TIMED_INFO)`: `[MAX_TAP_DANCE, MAX_COMBO,
 * MAX_COMBO_KEYS, MAX_MACRO, MAX_MACRO_STEPS, MAX_LEADER, MAX_LEADER_SEQ]`.
 */
export function packTimedInfo(): Uint8Array {
  const out = replyPayload();
  out[0] = MAX_TAP_DANCE;
  out[1] = MAX_COMBO;
  out[2] = MAX_COMBO_KEYS;
  out[3] = MAX_MACRO;
  out[4] = MAX_MACRO_STEPS;
  out[5] = MAX_LEADER;
  out[6] = MAX_LEADER_SEQ;
  return out;
}

/** A configured leader entry (`timed::LeaderCfg`); `seq` are the five raw u16 slots. */
export interface LeaderSample {
  seq: [number, number, number, number, number];
  len: number;
  action: number;
}

/**
 * `pack_leader`: `len`, the five sequence keycodes (u16 LE; unused slots zero) and
 * the action keycode. An empty slot is `len = 0`.
 */
export function packLeader(leader: LeaderSample | null): Uint8Array {
  const out = replyPayload();
  if (leader) {
    out[0] = leader.len;
    const view = new DataView(out.buffer, out.byteOffset, out.byteLength);
    leader.seq.forEach((k, i) => view.setUint16(1 + i * 2, k & 0xffff, true));
    view.setUint16(1 + MAX_LEADER_SEQ * 2, leader.action & 0xffff, true);
  }
  return out;
}

// === WIRELESS group ========================================================

/** Devs codes `wireless::Devs::from_u8` accepts (mod.rs): USB, BT1..3, 2.4G. */
const KNOWN_DEVS: readonly number[] = [0, 1, 2, 3, 6];
/** `MdState::Disconnected` code (wireless/md.rs). */
const MD_STATE_DISCONNECTED = 3;

/** The link snapshot `pack_wireless_state` samples from `crate::wireless`. */
export interface WirelessStateSample {
  devs: number;
  state: number;
  battery: number;
  version: number;
}

/** A representative connected-over-BT1 snapshot. */
export const DEFAULT_WIRELESS_STATE: WirelessStateSample = {
  devs: 1, // Devs::Bt1
  state: 2, // MdState::Connected
  battery: 80,
  version: 3,
};

/** `pack_wireless_state`: `[devs, state, battery, version]`. */
export function packWirelessState(s: WirelessStateSample = DEFAULT_WIRELESS_STATE): Uint8Array {
  const out = replyPayload();
  out[0] = s.devs & 0xff;
  out[1] = s.state & 0xff;
  out[2] = s.battery & 0xff;
  out[3] = s.version & 0xff;
  return out;
}

/** `wireless_dispatch(CMD_WLS_GET_BATTERY)`: `out[0] = battery`. */
export function packBattery(level: number): Uint8Array {
  const out = replyPayload();
  out[0] = level & 0xff;
  return out;
}

// === TEXT group ============================================================

/**
 * `autocorrect::AC_ENTRY_COUNT` (the compiled-in dictionary size, `build.rs` `DICT`)
 * — the entry count AUTOCORRECT_INFO reports. Mirrored here for the client tests.
 */
export const AUTOCORRECT_ENTRY_COUNT = 53;

/** `autocorrect::on_kcp(CMD_TEXT_AUTOCORRECT_INFO)`: `[enabled, count(2 LE)]`. */
export function packAutocorrectInfo(
  enabled: boolean,
  entryCount = AUTOCORRECT_ENTRY_COUNT,
): Uint8Array {
  const out = replyPayload();
  out[0] = enabled ? 1 : 0;
  out[1] = entryCount & 0xff;
  out[2] = (entryCount >> 8) & 0xff;
  return out;
}

// === UNICODE group =========================================================

/** `UNICODE_MAP_SLOTS` (features/unicode.rs): host-uploadable `UM(n)` codepoint slots. */
export const UNICODE_MAP_SLOTS = 16;
/** `UNICODE_MODE_COUNT` (features/unicode.rs): Linux / macOS / Windows senders. */
export const UNICODE_MODE_COUNT = 3;

/** The Unicode-input state `unicode::on_kcp` reports and mutates. */
export interface UnicodeStateSample {
  /** Active OS input mode (`MODE_LINUX` = 0 at power-on). */
  mode: number;
  /** Codepoint table, one per `UM(n)` slot; `0` is an empty slot. */
  map: number[];
}

/** A fresh power-on snapshot: Linux mode, all slots empty (the firmware's RAM default). */
export const DEFAULT_UNICODE_STATE: UnicodeStateSample = {
  mode: 0, // MODE_LINUX
  map: new Array<number>(UNICODE_MAP_SLOTS).fill(0),
};

/** `unicode::on_kcp(CMD_UNICODE_GET)`: `[active_mode, slot_count, mode_count]`. */
export function packUnicodeInfo(s: UnicodeStateSample = DEFAULT_UNICODE_STATE): Uint8Array {
  const out = replyPayload();
  out[0] = s.mode & 0xff;
  out[1] = UNICODE_MAP_SLOTS & 0xff;
  out[2] = UNICODE_MODE_COUNT & 0xff;
  return out;
}

// === Stateful dispatcher ===================================================

/**
 * A mutable stand-in for the device state the stateful handlers touch (KEYMAP,
 * RGB, CONFIG, BEHAVIOR and WIRELESS).
 */
export interface FakeDevice {
  /** `[layer][row][col]` raw keycodes; seeded to NO (0), like a blank keymap. */
  keymap: number[][][];
  telemetry: TelemetrySample;
  /** Rollover toggle (`crate::usb::nkro_enabled`); boots NKRO off like the firmware. */
  nkro: boolean;
  rgb: RgbStateSample;
  /** Zone table (`rgb.rs` `ZONES`), `ZONE_CAP` slots; GET_ZONE / SET_ZONE(_RANGE). */
  zones: ZoneSample[];
  /** WLS_GET_STATE snapshot; mutated by SET_MODE / PAIR like `devs_change`. */
  wireless: WirelessStateSample;
  /** Radio idle-sleep policy, recorded by SET_SLEEP_POLICY. */
  sleepPolicy: { bt: boolean; g2g4: boolean };
  /** SOCD table, `MAX_SOCD` slots (empty = null), like `behavior::SOCD`. */
  socd: (SocdPairSample | null)[];
  /** Override table, `MAX_OVERRIDES` slots (empty = null). */
  overrides: (KeyOverrideSample | null)[];
  /** Tap-dance table, `MAX_TAP_DANCE` slots (empty = null), like `timed::td`. */
  tapDance: (TapDanceSample | null)[];
  /** Combo table, `MAX_COMBO` slots (empty = null), like `timed::combos`. */
  combos: (ComboSample | null)[];
  /** Macro table, `MAX_MACRO` slots, like `timed::macros`. */
  macros: MacroSlotSample[];
  /** Slot currently being recorded into (`timed::record`), or null when idle. */
  recording: number | null;
  /** Flash storage descriptor; SAVE marks it valid, like `config::save_keymap`. */
  storage: StorageInfoSample;
  /** Matrix debounce config (matrix.rs live statics); GET/SET_DEBOUNCE. */
  debounce: DebounceSample;
  /** Runtime tunables (timed.rs live state); GET/SET_TUNING. */
  tuning: TuningSample;
  /** Leader sequence table, `MAX_LEADER` slots (empty = null), like `timed::leader_table`. */
  leader: (LeaderSample | null)[];
  /** Layer config (keymap.rs live state): the default `DF` layer and the tri-layer rule. */
  layerConfig: LayerConfigSample;
  /**
   * Per-feature runtime enable, keyed by `FeatureId` (`features::ENABLED` bitmap); every
   * feature defaults on like the firmware. The TEXT group's autocorrect flag is this map's
   * {@link FEATURE_ID_AUTOCORRECT} bit (one folded enable, surfaced in two groups).
   */
  featuresEnabled: Record<number, boolean>;
  /** Unicode-input state (features/unicode.rs RAM): active OS mode + the codepoint map. */
  unicode: UnicodeStateSample;
}

/** Build a fresh device: a zeroed keymap and the default group states. */
export function createFakeDevice(): FakeDevice {
  const keymap = Array.from({ length: LAYERS }, () =>
    Array.from({ length: NUM_ROWS }, () => new Array<number>(NUM_COLS).fill(0)),
  );
  return {
    keymap,
    telemetry: { ...DEFAULT_TELEMETRY },
    nkro: false,
    rgb: { ...DEFAULT_RGB_STATE },
    zones: DEFAULT_ZONES.map((z) => ({ ...z })),
    wireless: { ...DEFAULT_WIRELESS_STATE },
    sleepPolicy: { bt: true, g2g4: true },
    socd: new Array<SocdPairSample | null>(MAX_SOCD).fill(null),
    overrides: new Array<KeyOverrideSample | null>(MAX_OVERRIDES).fill(null),
    tapDance: new Array<TapDanceSample | null>(MAX_TAP_DANCE).fill(null),
    combos: new Array<ComboSample | null>(MAX_COMBO).fill(null),
    macros: Array.from({ length: MAX_MACRO }, emptyMacroSlot),
    recording: null,
    storage: { ...DEFAULT_STORAGE_INFO },
    debounce: { ...DEFAULT_DEBOUNCE },
    tuning: { ...DEFAULT_TUNING },
    leader: new Array<LeaderSample | null>(MAX_LEADER).fill(null),
    layerConfig: { ...DEFAULT_LAYER_CONFIG },
    featuresEnabled: Object.fromEntries(FEATURE_DEFS.map((f) => [f.id, true])),
    unicode: { mode: DEFAULT_UNICODE_STATE.mode, map: [...DEFAULT_UNICODE_STATE.map] },
  };
}

function inBounds(d: FakeDevice, layer: number, row: number, col: number): boolean {
  return layer < d.keymap.length && row < NUM_ROWS && col < NUM_COLS;
}

function keymapDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  // Request payload starts at req[2] (no STATUS byte on requests).
  switch (cmd) {
    case Cmd.GetKeycode: {
      const [layer, row, col] = [req[2], req[3], req[4]];
      if (!inBounds(d, layer, row, col)) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packKeycodeReply(d.keymap[layer][row][col])];
    }
    case Cmd.SetKeycode: {
      const [layer, row, col] = [req[2], req[3], req[4]];
      const kc = (req[5] | (req[6] << 8)) & 0xffff;
      if (!inBounds(d, layer, row, col)) return [Status.BadArg, replyPayload()];
      d.keymap[layer][row][col] = kc;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.GetLayerCount:
      return [Status.Ok, packLayerCount(d.keymap.length)];
    case Cmd.GetLayerConfig:
      return [Status.Ok, packLayerConfig(d.layerConfig)];
    case Cmd.SetLayerConfig: {
      const defaultLayer = req[2];
      const triEnabled = req[3] !== 0;
      const [triL1, triL2, triL3] = [req[4], req[5], req[6]];
      // Mirror `set_default_layer` + `set_tri_layer`: reject an out-of-range default
      // layer, or an enabled tri-layer with an out-of-range layer or `l1 == l2`.
      const layers = d.keymap.length;
      const triBad =
        triEnabled && (triL1 >= layers || triL2 >= layers || triL3 >= layers || triL1 === triL2);
      if (defaultLayer >= layers || triBad) return [Status.BadArg, replyPayload()];
      d.layerConfig = {
        defaultLayer,
        triEnabled,
        triL1: triEnabled ? triL1 : 0,
        triL2: triEnabled ? triL2 : 0,
        triL3: triEnabled ? triL3 : 0,
      };
      return [Status.Ok, replyPayload()];
    }
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

function rgbDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    case Cmd.RgbSetMode:
      if (req[2] < MODE_COUNT) {
        d.rgb.mode = req[2];
        return [Status.Ok, replyPayload()];
      }
      return [Status.BadArg, replyPayload()];
    case Cmd.RgbSetHsv:
      [d.rgb.hue, d.rgb.sat, d.rgb.val] = [req[2], req[3], req[4]];
      return [Status.Ok, replyPayload()];
    case Cmd.RgbSetBrightness:
      d.rgb.brightness = req[2];
      return [Status.Ok, replyPayload()];
    case Cmd.RgbSetSpeed:
      d.rgb.speed = req[2];
      return [Status.Ok, replyPayload()];
    case Cmd.RgbSetEnabled:
      if (req[2] === 0 || req[2] === 1) {
        d.rgb.enabled = req[2] === 1;
        return [Status.Ok, replyPayload()];
      }
      return [Status.BadArg, replyPayload()];
    case Cmd.RgbSetIndicators:
      if (req[2] === 0 || req[2] === 1) {
        d.rgb.indicators = req[2] === 1;
        return [Status.Ok, replyPayload()];
      }
      return [Status.BadArg, replyPayload()];
    case Cmd.RgbGetState:
      return [Status.Ok, packRgbState(d.rgb)];
    case Cmd.RgbListModes:
      return [Status.Ok, packRgbModeList(req[2])];
    case Cmd.RgbGetZones:
      return [Status.Ok, packZones()];
    case Cmd.RgbGetZone: {
      const id = req[2];
      if (id >= ZONE_CAP) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packZone(id, d.zones[id])];
    }
    case Cmd.RgbSetZone: {
      // [id, flags, mode, hue, sat, val, brightness, speed]; id and mode are
      // range-checked, like the firmware's `set_zone`.
      const id = req[2];
      if (id >= ZONE_CAP || req[4] >= MODE_COUNT) return [Status.BadArg, replyPayload()];
      const flags = req[3];
      const current = d.zones[id];
      // Enabling a zone (an off->on transition of ENABLED) must keep the lit ranges
      // disjoint — the range is set separately, so check the zone's current range.
      if (
        (flags & ZONE_FLAG_ENABLED) !== 0 &&
        (current.flags & ZONE_FLAG_ENABLED) === 0 &&
        zoneRangeOverlaps(d.zones, id, current.start, current.count)
      )
        return [Status.BadArg, replyPayload()];
      d.zones[id] = {
        ...d.zones[id],
        flags,
        mode: req[4],
        hue: req[5],
        sat: req[6],
        val: req[7],
        brightness: req[8],
        speed: req[9],
      };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.RgbSetZoneRange: {
      // [id, start:u16le, count:u16le]; the range must fit the chain and stay disjoint
      // from the other lit zones, like `set_zone_range` + `zone_range_overlaps`.
      const id = req[2];
      const start = req[3] | (req[4] << 8);
      const count = req[5] | (req[6] << 8);
      if (id >= ZONE_CAP || start + count > LED_COUNT) return [Status.BadArg, replyPayload()];
      if (zoneRangeOverlaps(d.zones, id, start, count)) return [Status.BadArg, replyPayload()];
      d.zones[id] = { ...d.zones[id], start, count };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.RgbDirect: {
      // [offset:u16le, len, rgb[len*3]]; framing + chain bounds validated like the
      // firmware's `direct_write`. The fixture has no render, so it only validates.
      const offset = req[2] | (req[3] << 8);
      const len = req[4];
      const end = 5 + len * 3;
      if (end > req.length || offset + len > LED_COUNT) return [Status.BadArg, replyPayload()];
      return [Status.Ok, replyPayload()];
    }
    case Cmd.RgbSetZoneSync: {
      // [id, target]; target 0xFF clears. Validate id/target and reject a sync cycle,
      // like the firmware's `set_zone_sync`.
      const id = req[2];
      const target = req[3];
      if (id >= ZONE_CAP) return [Status.BadArg, replyPayload()];
      if (target !== ZONE_SYNC_NONE && (target >= ZONE_CAP || target === id))
        return [Status.BadArg, replyPayload()];
      if (target !== ZONE_SYNC_NONE && syncWouldCycle(d.zones, id, target))
        return [Status.BadArg, replyPayload()];
      d.zones[id] = { ...d.zones[id], syncTo: target };
      return [Status.Ok, replyPayload()];
    }
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

/** Whether placing zone `id` at `start..start+count` overlaps another enabled, non-empty
 * zone — the `zone_range_overlaps` mirror (an empty range overlaps nothing). */
function zoneRangeOverlaps(zones: ZoneSample[], id: number, start: number, count: number): boolean {
  if (count === 0) return false;
  return zones.some(
    (z, other) =>
      other !== id &&
      (z.flags & ZONE_FLAG_ENABLED) !== 0 &&
      z.count !== 0 &&
      start < z.start + z.count &&
      z.start < start + count,
  );
}

/** Whether linking zone `id` to `target` would close a sync cycle — the `set_zone_sync`
 * cycle-guard mirror (walk the chain from `target`; reaching `id` is a cycle). */
function syncWouldCycle(zones: ZoneSample[], id: number, target: number): boolean {
  let cur = target;
  for (let hops = 0; hops < ZONE_CAP; hops += 1) {
    if (cur === id) return true;
    const next = zones[cur].syncTo;
    if (next >= ZONE_CAP) break;
    cur = next;
  }
  return false;
}

function configDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    case Cmd.ConfigSave:
      // A successful flash write + read-back leaves a valid current-version blob;
      // `config::save` stamps it with `SCHEMA_VERSION`, which GET_STORAGE_INFO reads.
      d.storage.valid = true;
      d.storage.version = SCHEMA_VERSION;
      return [Status.Ok, replyPayload()];
    case Cmd.ConfigLoadDefaults:
      // Resets the live RAM keymap to defaults; flash (storage) is untouched.
      return [Status.Ok, replyPayload()];
    case Cmd.ConfigGetStorageInfo:
      return [Status.Ok, packStorageInfo(d.storage)];
    case Cmd.ConfigGetDebounce:
      return [Status.Ok, packDebounce(d.debounce)];
    case Cmd.ConfigSetDebounce: {
      const [algorithm, interval] = [req[2], req[3]];
      const known = algorithm === DEBOUNCE_SYMMETRIC || algorithm === DEBOUNCE_EAGER;
      // `matrix::set_debounce` rejects an unknown algorithm or a zero interval.
      if (!known || interval === 0) return [Status.BadArg, replyPayload()];
      d.debounce = { algorithm, interval };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.ConfigGetTuning:
      return [Status.Ok, packTuning(d.tuning)];
    case Cmd.ConfigSetTuning: {
      const autoShiftTimeoutMs = reqU16(req, 3);
      const leaderTimeoutMs = reqU16(req, 5);
      const tapHoldTermMs = reqU16(req, 7);
      const flags = req[9];
      const quickTapTermMs = reqU16(req, 10);
      // `config_dispatch` rejects a zero auto-shift, leader or tap-hold term (a zero
      // quick-tap window is valid and disables quick-tap).
      if (autoShiftTimeoutMs === 0 || leaderTimeoutMs === 0 || tapHoldTermMs === 0) {
        return [Status.BadArg, replyPayload()];
      }
      d.tuning = {
        autoShiftEnabled: req[2] !== 0,
        autoShiftTimeoutMs,
        leaderTimeoutMs,
        tapHoldTermMs,
        permissiveHold: (flags & 1) !== 0,
        holdOnOtherKeyPress: (flags & 2) !== 0,
        retroTapping: (flags & 4) !== 0,
        chordalHold: (flags & 8) !== 0,
        quickTapTermMs,
      };
      return [Status.Ok, replyPayload()];
    }
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

/** Read a little-endian u16 from a request frame at byte `i` (`req[i..i+2]`). */
function reqU16(req: Uint8Array, i: number): number {
  return (req[i] | (req[i + 1] << 8)) & 0xffff;
}

function hidKroDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    case Cmd.GetKro:
      return [Status.Ok, packKro(d.nkro)];
    case Cmd.SetKro:
      if (req[2] === 0 || req[2] === 1) {
        d.nkro = req[2] === 1;
        return [Status.Ok, replyPayload()];
      }
      return [Status.BadArg, replyPayload()];
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

function macroDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  // Request payload starts at req[2] (no STATUS byte on requests).
  switch (cmd) {
    case Cmd.MacroInfo:
      return [Status.Ok, packMacroInfo(d.macros)];
    case Cmd.MacroSetStep: {
      const [macro, step] = [req[2], req[3]];
      if (macro >= MAX_MACRO || step >= MAX_MACRO_STEPS) return [Status.BadArg, replyPayload()];
      const slot = d.macros[macro];
      slot.steps[step] = { kc: reqU16(req, 4), down: req[6] !== 0, delayMs: reqU16(req, 7) };
      if (step >= slot.len) slot.len = step + 1;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.MacroGetStep: {
      const [macro, step] = [req[2], req[3]];
      if (macro >= MAX_MACRO || step >= MAX_MACRO_STEPS) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packMacroStep(d.macros[macro], step)];
    }
    case Cmd.MacroClear: {
      const macro = req[2];
      if (macro === CLEAR_ALL) {
        d.macros = Array.from({ length: MAX_MACRO }, emptyMacroSlot);
        return [Status.Ok, replyPayload()];
      }
      if (macro >= MAX_MACRO) return [Status.BadArg, replyPayload()];
      d.macros[macro] = emptyMacroSlot();
      return [Status.Ok, replyPayload()];
    }
    case Cmd.MacroPlay: {
      const macro = req[2];
      if (macro >= MAX_MACRO || d.macros[macro].len === 0) return [Status.BadArg, replyPayload()];
      return [Status.Ok, replyPayload()];
    }
    case Cmd.MacroRecordStart: {
      // Mirror `timed::record_start`: clear the slot and arm the recorder. The
      // device captures matrix edges into the slot live; this fake has no matrix,
      // so it models only the slot wipe and the recording-state flag.
      const macro = req[2];
      if (macro >= MAX_MACRO) return [Status.BadArg, replyPayload()];
      d.macros[macro] = emptyMacroSlot();
      d.recording = macro;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.MacroRecordStop:
      // Mirror `timed::record_stop`: always Ok, a no-op when idle.
      d.recording = null;
      return [Status.Ok, replyPayload()];
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

function behaviorDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  // Request payload starts at req[2] (no STATUS byte on requests).
  switch (cmd) {
    case Cmd.SocdSet: {
      const index = req[2];
      const mode = req[7];
      const modeOk = mode === 0 || mode === 1 || mode === 2;
      if (index >= MAX_SOCD || !modeOk) return [Status.BadArg, replyPayload()];
      d.socd[index] = {
        a: (req[3] | (req[4] << 8)) & 0xffff,
        b: (req[5] | (req[6] << 8)) & 0xffff,
        mode,
      };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.SocdClear: {
      const index = req[2];
      if (index === CLEAR_ALL) {
        d.socd.fill(null);
        return [Status.Ok, replyPayload()];
      }
      if (index >= MAX_SOCD) return [Status.BadArg, replyPayload()];
      d.socd[index] = null;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.SocdGet: {
      const index = req[2];
      if (index >= MAX_SOCD) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packSocdPair(d.socd[index])];
    }
    case Cmd.OverrideSet: {
      const index = req[2];
      if (index >= MAX_OVERRIDES) return [Status.BadArg, replyPayload()];
      d.overrides[index] = {
        trigger: (req[3] | (req[4] << 8)) & 0xffff,
        triggerMods: req[5],
        replacement: (req[6] | (req[7] << 8)) & 0xffff,
        replacementMods: req[8],
        layerMask: (req[9] | (req[10] << 8)) & 0xffff,
        enabled: req[11] !== 0,
      };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.OverrideClear: {
      const index = req[2];
      if (index === CLEAR_ALL) {
        d.overrides.fill(null);
        return [Status.Ok, replyPayload()];
      }
      if (index >= MAX_OVERRIDES) return [Status.BadArg, replyPayload()];
      d.overrides[index] = null;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.OverrideGet: {
      const index = req[2];
      if (index >= MAX_OVERRIDES) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packOverride(d.overrides[index])];
    }
    case Cmd.BehaviorInfo:
      return [Status.Ok, packBehaviorInfo()];
    case Cmd.TapdanceSet: {
      const index = req[2];
      if (index >= MAX_TAP_DANCE) return [Status.BadArg, replyPayload()];
      d.tapDance[index] = {
        tap: reqU16(req, 3),
        hold: reqU16(req, 5),
        double: reqU16(req, 7),
        termMs: reqU16(req, 9),
      };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.TapdanceGet: {
      const index = req[2];
      if (index >= MAX_TAP_DANCE) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packTapdance(d.tapDance[index])];
    }
    case Cmd.TapdanceClear: {
      const index = req[2];
      if (index === CLEAR_ALL) {
        d.tapDance.fill(null);
        return [Status.Ok, replyPayload()];
      }
      if (index >= MAX_TAP_DANCE) return [Status.BadArg, replyPayload()];
      d.tapDance[index] = null;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.ComboSet: {
      const index = req[2];
      const len = req[3];
      const flags = req[16];
      const keys: [number, number, number, number] = [
        reqU16(req, 4),
        reqU16(req, 6),
        reqU16(req, 8),
        reqU16(req, 10),
      ];
      // Mirror `combo_set`: reject a bad index/len, an unknown flag bit, the contradictory
      // must-hold + must-tap pair, or a duplicate among the live member keys (a chord must
      // be distinct keys).
      const bothHoldAndTap =
        (flags & (COMBO_FLAG_MUST_HOLD | COMBO_FLAG_MUST_TAP)) ===
        (COMBO_FLAG_MUST_HOLD | COMBO_FLAG_MUST_TAP);
      const members = keys.slice(0, len);
      const hasDuplicate = new Set(members).size !== members.length;
      if (
        index >= MAX_COMBO ||
        len < MIN_COMBO_KEYS ||
        len > MAX_COMBO_KEYS ||
        (flags & ~COMBO_FLAG_MASK) !== 0 ||
        bothHoldAndTap ||
        hasDuplicate
      ) {
        return [Status.BadArg, replyPayload()];
      }
      d.combos[index] = {
        keys,
        len,
        action: reqU16(req, 12),
        termMs: reqU16(req, 14),
        flags,
      };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.ComboGet: {
      const index = req[2];
      if (index >= MAX_COMBO) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packCombo(d.combos[index])];
    }
    case Cmd.ComboClear: {
      const index = req[2];
      if (index === CLEAR_ALL) {
        d.combos.fill(null);
        return [Status.Ok, replyPayload()];
      }
      if (index >= MAX_COMBO) return [Status.BadArg, replyPayload()];
      d.combos[index] = null;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.TimedInfo:
      return [Status.Ok, packTimedInfo()];
    case Cmd.LeaderSet: {
      const index = req[2];
      const len = req[3];
      // `timed::leader_set` rejects an out-of-range index or len above the cap.
      if (index >= MAX_LEADER || len > MAX_LEADER_SEQ) return [Status.BadArg, replyPayload()];
      d.leader[index] =
        len === 0
          ? null
          : {
              seq: [
                reqU16(req, 4),
                reqU16(req, 6),
                reqU16(req, 8),
                reqU16(req, 10),
                reqU16(req, 12),
              ],
              len,
              action: reqU16(req, 14),
            };
      return [Status.Ok, replyPayload()];
    }
    case Cmd.LeaderGet: {
      const index = req[2];
      if (index >= MAX_LEADER) return [Status.BadArg, replyPayload()];
      return [Status.Ok, packLeader(d.leader[index])];
    }
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

/** Mirror of `wireless::devs_change`: a mode change or a reset drops the link. */
function devsChange(d: FakeDevice, newDevs: number, reset: boolean): void {
  if (d.wireless.devs !== newDevs || reset) {
    d.wireless.state = MD_STATE_DISCONNECTED;
  }
  d.wireless.devs = newDevs;
}

function wirelessDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    case Cmd.WlsGetState:
      return [Status.Ok, packWirelessState(d.wireless)];
    case Cmd.WlsSetMode: {
      const devs = req[2];
      if (!KNOWN_DEVS.includes(devs)) return [Status.BadArg, replyPayload()];
      devsChange(d, devs, false);
      return [Status.Ok, replyPayload()];
    }
    case Cmd.WlsPair:
      devsChange(d, d.wireless.devs, true);
      return [Status.Ok, replyPayload()];
    case Cmd.WlsUnpair:
      return [Status.Ok, replyPayload()];
    case Cmd.WlsSetSleepPolicy:
      d.sleepPolicy = { bt: req[2] !== 0, g2g4: req[3] !== 0 };
      return [Status.Ok, replyPayload()];
    case Cmd.WlsGetBattery:
      return [Status.Ok, packBattery(d.wireless.battery)];
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

function textDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    // Autocorrect's enable is the central bitmap's FEATURE_ID_AUTOCORRECT bit, so the TEXT
    // group reads/writes the same state the FEATURES group enumerates and toggles.
    case Cmd.TextAutocorrectInfo:
      return [Status.Ok, packAutocorrectInfo(d.featuresEnabled[FEATURE_ID_AUTOCORRECT])];
    case Cmd.TextAutocorrectSet:
      if (req[2] === 0 || req[2] === 1) {
        d.featuresEnabled[FEATURE_ID_AUTOCORRECT] = req[2] === 1;
        return [Status.Ok, replyPayload()];
      }
      return [Status.BadArg, replyPayload()];
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

/** Mirror of `unicode::on_kcp`: report the mode/slot/mode counts, set the mode, fill a slot. */
function unicodeDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    case Cmd.UnicodeGet:
      return [Status.Ok, packUnicodeInfo(d.unicode)];
    case Cmd.UnicodeSetMode: {
      const mode = req[2];
      if (mode >= UNICODE_MODE_COUNT) return [Status.BadArg, replyPayload()];
      d.unicode.mode = mode;
      return [Status.Ok, replyPayload()];
    }
    case Cmd.UnicodeSetMap: {
      const slot = req[2];
      if (slot >= UNICODE_MAP_SLOTS) return [Status.BadArg, replyPayload()];
      // Codepoint is a little-endian u32 at req[3..7], as `encodeSetMapArgs` lays it out.
      d.unicode.map[slot] = (req[3] | (req[4] << 8) | (req[5] << 16) | (req[6] << 24)) >>> 0;
      return [Status.Ok, replyPayload()];
    }
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

/**
 * Pack a GET_FEATURES page from index `start`, mirroring `features::pack_features`:
 * `out[0]` = total count, `out[1]` = records in this page, then `[id, enabled, name_len,
 * name_bytes]` while the next record fits the 29-byte payload — so the fixture pages
 * exactly as the firmware does and the client's paging loop terminates identically.
 */
function packFeaturesPage(start: number, d: FakeDevice): Uint8Array {
  const out = replyPayload();
  out[0] = FEATURE_DEFS.length;
  let pos = 2;
  let page = 0;
  for (let i = start; i < FEATURE_DEFS.length; i += 1) {
    const def = FEATURE_DEFS[i];
    const name = new TextEncoder().encode(def.name);
    if (pos + 3 + name.length > out.length) break;
    out[pos] = def.id;
    out[pos + 1] = d.featuresEnabled[def.id] ? 1 : 0;
    out[pos + 2] = name.length;
    out.set(name, pos + 3);
    pos += 3 + name.length;
    page += 1;
  }
  out[1] = page;
  return out;
}

/** Mirror of `features::features_dispatch`: enumerate the registry, toggle one feature. */
function featuresDispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {
  switch (cmd) {
    case Cmd.GetFeatures:
      return [Status.Ok, packFeaturesPage(req[2], d)];
    case Cmd.SetFeatureEnabled: {
      const id = req[2];
      const on = req[3];
      const def = FEATURE_DEFS.find((f) => f.id === id);
      if (!def || (on !== 0 && on !== 1) || (on === 0 && def.alwaysOn)) {
        return [Status.BadArg, replyPayload()];
      }
      d.featuresEnabled[id] = on === 1;
      return [Status.Ok, replyPayload()];
    }
    default:
      return [Status.BadCmd, replyPayload()];
  }
}

/**
 * Mirror of `kcp.rs::handle`: build the 32-byte reply for a request, setting
 * REPLY_FLAG, echoing SEQ, and writing STATUS + payload. Dispatches the wired
 * groups (INFO, KEYMAP, TELEMETRY, HID_KRO, CONFIG, MACRO, RGB, BEHAVIOR,
 * WIRELESS, TEXT, UNICODE, FEATURES) against the optional mutable `device` so set/get round-trips through
 * the codec, exactly as the firmware does; an unknown group answers UNSUPPORTED
 * and an unknown op within a group BAD_CMD. (The SYSTEM group resets the MCU and
 * never replies, so it is not a request/reply group this fake can model.)
 */
export function fakeFirmwareHandle(req: Uint8Array, device = createFakeDevice()): Uint8Array {
  const cmd = req[0];
  const seq = req[1];
  const reply = new Uint8Array(32);
  reply[0] = cmd | REPLY_FLAG;
  reply[1] = seq;

  let status = Status.Ok;
  let payload = replyPayload();

  switch (cmd >> 4) {
    case Group.Info:
      switch (cmd) {
        case Cmd.GetVersion:
          payload = packProtocolVersion();
          break;
        case Cmd.GetCapabilities:
          payload = packCapabilities();
          break;
        case Cmd.GetDeviceInfo:
          payload = packDeviceInfo();
          break;
        default:
          status = Status.BadCmd;
      }
      break;
    case Group.Keymap:
      [status, payload] = keymapDispatch(cmd, req, device);
      break;
    case Group.Telemetry:
      if (cmd === Cmd.GetTelemetry) {
        payload = packTelemetry(device.telemetry);
      } else {
        status = Status.BadCmd;
      }
      break;
    case Group.HidKro:
      [status, payload] = hidKroDispatch(cmd, req, device);
      break;
    case Group.Macro:
      [status, payload] = macroDispatch(cmd, req, device);
      break;
    case Group.Rgb:
      [status, payload] = rgbDispatch(cmd, req, device);
      break;
    case Group.Config:
      [status, payload] = configDispatch(cmd, req, device);
      break;
    case Group.Behavior:
      [status, payload] = behaviorDispatch(cmd, req, device);
      break;
    case Group.Wireless:
      [status, payload] = wirelessDispatch(cmd, req, device);
      break;
    case Group.Text:
      [status, payload] = textDispatch(cmd, req, device);
      break;
    case Group.Unicode:
      [status, payload] = unicodeDispatch(cmd, req, device);
      break;
    case Group.Features:
      [status, payload] = featuresDispatch(cmd, req, device);
      break;
    default:
      status = Status.Unsupported;
  }

  reply[2] = status;
  reply.set(payload, 3);
  return reply;
}
