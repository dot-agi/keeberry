// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * kcp protocol constants — the wire contract, kept byte-for-byte in lockstep
 * with the firmware (`firmware/src/kcp.rs`).
 *
 * One 32-byte raw-HID report is exactly one kcp message; there is no report ID.
 *
 *   request : [0]=CMD       [1]=SEQ  [2..32]=payload (30 bytes)
 *   reply   : [0]=CMD|0x80  [1]=SEQ  [2]=STATUS  [3..32]=payload (29 bytes)
 *
 * The CMD high nibble selects the command group; the low nibble selects the
 * operation. Replies set bit 7 of CMD (`REPLY_FLAG`) and echo the request SEQ,
 * so a reply pairs with its request on `reply.cmd === (req.cmd | REPLY_FLAG)`
 * and `reply.seq === req.seq` for every group.
 */

/** Raw-HID vendor usage page the kcp interface lives on (QMK-style). */
export const USAGE_PAGE = 0xff60;
/** Raw-HID vendor usage of the kcp interface. */
export const USAGE = 0x61;

/** Length of one kcp message: a single 32-byte HID report (IN and OUT). */
export const MSG_LEN = 32;
/** Bit OR-ed into the CMD byte to mark a frame as a reply. */
export const REPLY_FLAG = 0x80;

/** Byte offsets within a frame (mirror of `kcp.rs`). */
export const CMD_IDX = 0;
export const SEQ_IDX = 1;
/** STATUS byte, present in replies only. */
export const STATUS_IDX = 2;
/** First payload byte of a reply (after CMD, SEQ, STATUS): 29 bytes. */
export const REPLY_PAYLOAD_IDX = 3;
/** First payload byte of a request (after CMD, SEQ): 30 bytes. */
export const REQ_PAYLOAD_IDX = 2;

/** Bytes available to a request payload (`MSG_LEN - REQ_PAYLOAD_IDX`). */
export const REQ_PAYLOAD_LEN = MSG_LEN - REQ_PAYLOAD_IDX;
/** Bytes available to a reply payload (`MSG_LEN - REPLY_PAYLOAD_IDX`). */
export const REPLY_PAYLOAD_LEN = MSG_LEN - REPLY_PAYLOAD_IDX;

/**
 * Outcome of a request, returned in reply byte [2]. Numbering is fixed by the
 * kcp spec (`enum Status` in `kcp.rs`).
 */
export enum Status {
  Ok = 0,
  BadCmd = 1,
  BadArg = 2,
  Busy = 3,
  Unsupported = 4,
}

const STATUS_LABELS: Record<Status, string> = {
  [Status.Ok]: 'OK',
  [Status.BadCmd]: 'Bad command',
  [Status.BadArg]: 'Bad argument',
  [Status.Busy]: 'Busy',
  [Status.Unsupported]: 'Unsupported',
};

/** Human-readable label for a STATUS byte (handles unknown values). */
export function statusLabel(status: number): string {
  return STATUS_LABELS[status as Status] ?? `Unknown status (${status})`;
}

/**
 * Command groups: the high nibble of CMD, and the bit index of the group in the
 * capabilities bitmask (present group `g` sets bit `g`). Mirrors `mod group`
 * in `kcp.rs`.
 */
export const Group = {
  Info: 0x0,
  Keymap: 0x1,
  Telemetry: 0x2,
  HidKro: 0x3,
  Config: 0x4,
  Macro: 0x5,
  Rgb: 0x6,
  Behavior: 0x7,
  Wireless: 0x8,
  Text: 0x9,
  Unicode: 0xa,
  Features: 0xd,
  System: 0xf,
} as const;

/**
 * Operation codes for the wired command groups (mirror of `kcp.rs`). The high
 * nibble is the {@link Group}; the low nibble the operation within it.
 */
export const Cmd = {
  // --- INFO (0x0x) ---
  /** INFO 0x00 — protocol version. Reply payload `[major, minor]`. */
  GetVersion: 0x00,
  /** INFO 0x01 — capabilities bitmask (little-endian u32). */
  GetCapabilities: 0x01,
  /** INFO 0x02 — static device info (see `info.ts` for the layout). */
  GetDeviceInfo: 0x02,

  // --- KEYMAP (0x1x) ---
  /** KEYMAP 0x10 — get keycode. Req `[layer, row, col]`; reply u16 LE. */
  GetKeycode: 0x10,
  /** KEYMAP 0x11 — set keycode. Req `[layer, row, col, kc_lo, kc_hi]`. */
  SetKeycode: 0x11,
  /** KEYMAP 0x12 — layer count. Reply `[LAYERS]`. */
  GetLayerCount: 0x12,
  /**
   * KEYMAP 0x13 — get the layer config. Reply `[default_layer, tri_enabled, tri_l1,
   * tri_l2, tri_l3]` (the persistent `DF` base layer and the tri-layer rule).
   */
  GetLayerConfig: 0x13,
  /** KEYMAP 0x14 — set the layer config. Req `[default_layer, tri_enabled, tri_l1, tri_l2, tri_l3]`. */
  SetLayerConfig: 0x14,

  // --- TELEMETRY (0x2x) ---
  /** TELEMETRY 0x20 — live snapshot (see `telemetry.ts` for the layout). */
  GetTelemetry: 0x20,

  // --- HID_KRO (0x3x) ---
  /** HID_KRO 0x30 — get the rollover mode. Reply `[nkro_enabled]` (1 = NKRO). */
  GetKro: 0x30,
  /** HID_KRO 0x31 — set the rollover mode. Req `[0|1]` (0 = boot 6KRO, 1 = NKRO). */
  SetKro: 0x31,

  // --- CONFIG (0x4x) ---
  /** CONFIG 0x40 — persist the complete live config to flash. Ok or Busy. */
  ConfigSave: 0x40,
  /** CONFIG 0x41 — reset the complete live config to firmware defaults. */
  ConfigLoadDefaults: 0x41,
  /** CONFIG 0x42 — describe the persistence region (see `config.ts`). */
  ConfigGetStorageInfo: 0x42,
  /** CONFIG 0x43 — read the matrix debounce config. Reply `[algorithm, interval]`. */
  ConfigGetDebounce: 0x43,
  /** CONFIG 0x44 — set the matrix debounce config. Req `[algorithm, interval]`. */
  ConfigSetDebounce: 0x44,
  /** CONFIG 0x45 — read the runtime tunables. Reply `[as_on, as_timeout(2), leader_timeout(2)]`. */
  ConfigGetTuning: 0x45,
  /** CONFIG 0x46 — set the runtime tunables. Req `[as_on, as_timeout(2), leader_timeout(2)]`. */
  ConfigSetTuning: 0x46,

  // --- MACRO (0x5x) ---
  /** MACRO 0x50 — table capacities + used bitmap. Reply `[max, steps, used(4 LE)]`. */
  MacroInfo: 0x50,
  /** MACRO 0x51 — set one step. Req `[macro, step, kc_lo, kc_hi, down, delay_lo, delay_hi]`. */
  MacroSetStep: 0x51,
  /** MACRO 0x52 — read one step. Req `[macro, step]`; reply `[present, …, len]`. */
  MacroGetStep: 0x52,
  /** MACRO 0x53 — clear a macro (or all with index 0xFF). Req `[macro]`. */
  MacroClear: 0x53,
  /** MACRO 0x54 — play a macro now. Req `[macro]`. */
  MacroPlay: 0x54,
  /** MACRO 0x55 — start on-board recording into a slot (clears it). Req `[macro]`. */
  MacroRecordStart: 0x55,
  /** MACRO 0x56 — stop on-board recording. No payload. */
  MacroRecordStop: 0x56,

  // --- RGB (0x6x) ---
  /** RGB 0x60 — set effect mode. Req `[mode_id]` (0..MODE_COUNT). */
  RgbSetMode: 0x60,
  /** RGB 0x61 — set effect colour. Req `[h, s, v]`. */
  RgbSetHsv: 0x61,
  /** RGB 0x62 — set master brightness. Req `[val]` (0..=255). */
  RgbSetBrightness: 0x62,
  /** RGB 0x63 — enable/disable RGB. Req `[0|1]`. */
  RgbSetEnabled: 0x63,
  /** RGB 0x64 — get live state (see `rgb.ts` for the layout). */
  RgbGetState: 0x64,
  /** RGB 0x65 — list effect modes (paginated). Request `[start]`; reply
   * `[total, page_len, id0, id1, …]`. The client pages until it has all `total`. */
  RgbListModes: 0x65,
  /** RGB 0x66 — set animation speed. Req `[speed]` (0..=255). */
  RgbSetSpeed: 0x66,
  /** RGB 0x67 — enable/disable the status-indicator overlay. Req `[0|1]`. */
  RgbSetIndicators: 0x67,
  /** RGB 0x68 — get the zone-table summary. Reply `[zone_count, zone_cap]`. */
  RgbGetZones: 0x68,
  /** RGB 0x69 — get one zone. Req `[id]`; reply `[id, flags, mode, h, s, v, bright, speed, start(2), count(2), syncTo (0=none, else zone id+1)]`. */
  RgbGetZone: 0x69,
  /** RGB 0x6A — set one zone's effect. Req `[id, flags, mode, h, s, v, brightness, speed]`. */
  RgbSetZone: 0x6a,
  /** RGB 0x6B — set one zone's LED range (must stay disjoint from the other lit zones). Req `[id, start(2 LE), count(2 LE)]`. */
  RgbSetZoneRange: 0x6b,
  /**
   * RGB 0x6C — stream a host-rendered frame chunk (Direct-mode scaffold). Req
   * `[offset(2 LE), len, rgb[len*3]]`; bypasses the base+zone effects until a ~1 s
   * watchdog reverts to them. The host streaming engine is deferred (vN).
   */
  RgbDirect: 0x6c,
  /** RGB 0x6D — set one zone's sync source. Req `[id, target]` (target 0xFF clears); a bad target or a sync cycle is BadArg. */
  RgbSetZoneSync: 0x6d,

  // --- BEHAVIOR (0x7x) ---
  /** BEHAVIOR 0x70 — set a SOCD pair. Req `[index, a_lo, a_hi, b_lo, b_hi, mode]`. */
  SocdSet: 0x70,
  /** BEHAVIOR 0x71 — clear a SOCD pair (or all with index 0xFF). Req `[index]`. */
  SocdClear: 0x71,
  /** BEHAVIOR 0x72 — get a SOCD pair. Req `[index]`; reply `[present, a, b, mode]`. */
  SocdGet: 0x72,
  /** BEHAVIOR 0x73 — set a key override (see `behavior.ts` for the layout). */
  OverrideSet: 0x73,
  /** BEHAVIOR 0x74 — clear a key override (or all with index 0xFF). Req `[index]`. */
  OverrideClear: 0x74,
  /** BEHAVIOR 0x75 — get a key override. Req `[index]`; reply `[present, …]`. */
  OverrideGet: 0x75,
  /** BEHAVIOR 0x76 — table capacities. Reply `[MAX_SOCD, MAX_OVERRIDES]`. */
  BehaviorInfo: 0x76,
  /** BEHAVIOR 0x77 — set a tap-dance entry. Req `[index, tap, hold, double, term]` (LE). */
  TapdanceSet: 0x77,
  /** BEHAVIOR 0x78 — get a tap-dance entry. Req `[index]`; reply `[present, …]`. */
  TapdanceGet: 0x78,
  /** BEHAVIOR 0x79 — clear a tap-dance entry (or all with index 0xFF). Req `[index]`. */
  TapdanceClear: 0x79,
  /** BEHAVIOR 0x7A — set a combo. Req `[index, len, k0..k3, action, term, flags]` (LE). */
  ComboSet: 0x7a,
  /** BEHAVIOR 0x7B — get a combo. Req `[index]`; reply `[present, len, …, term, flags]`. */
  ComboGet: 0x7b,
  /** BEHAVIOR 0x7C — clear a combo (or all with index 0xFF). Req `[index]`. */
  ComboClear: 0x7c,
  /**
   * BEHAVIOR 0x7D — timed-engine capacities. Reply `[td, combo, comboKeys, macro,
   * steps, maxLeader, maxLeaderSeq]`.
   */
  TimedInfo: 0x7d,
  /** BEHAVIOR 0x7E — set a leader entry. Req `[index, len, s0..s4, action]` (LE). */
  LeaderSet: 0x7e,
  /** BEHAVIOR 0x7F — get a leader entry. Req `[index]`; reply `[len, s0..s4, action]`. */
  LeaderGet: 0x7f,

  // --- WIRELESS (0x8x) ---
  /** WIRELESS 0x80 — link snapshot. Reply `[devs, state, battery, version]`. */
  WlsGetState: 0x80,
  /** WIRELESS 0x81 — select the output transport. Req `[devs]`. */
  WlsSetMode: 0x81,
  /** WIRELESS 0x82 — (re)pair the current transport. No payload. */
  WlsPair: 0x82,
  /** WIRELESS 0x83 — clear the active channel's bond. No payload. */
  WlsUnpair: 0x83,
  /** WIRELESS 0x84 — set the radio sleep policy. Req `[enable_bt, enable_2g4]`. */
  WlsSetSleepPolicy: 0x84,
  /** WIRELESS 0x85 — battery level + refresh. Reply `[battery]`. */
  WlsGetBattery: 0x85,

  // --- TEXT (0x9x) ---
  /** TEXT 0x90 — autocorrect state. Reply `[enabled, count_lo, count_hi]` (entry count u16 LE). */
  TextAutocorrectInfo: 0x90,
  /** TEXT 0x91 — enable/disable autocorrect. Req `[0|1]`; applied live, persisted by CONFIG.SAVE. */
  TextAutocorrectSet: 0x91,

  // --- UNICODE (0xAx) ---
  /** UNICODE 0xA0 — get the Unicode-input state. Reply `[activeMode, slotCount, modeCount]`. */
  UnicodeGet: 0xa0,
  /** UNICODE 0xA1 — set the active OS input mode. Req `[mode]` (0 = Linux, 1 = macOS, 2 = Windows). */
  UnicodeSetMode: 0xa1,
  /** UNICODE 0xA2 — upload one codepoint slot. Req `[slot, cp(4 LE)]`; a 0 codepoint clears it. */
  UnicodeSetMap: 0xa2,

  // --- FEATURES (0xDx) ---
  /**
   * FEATURES 0xD0 — enumerate the registered features (paged). Req `[start]`; reply
   * `[count, page_len, {id, enabled, name_len, name_bytes}…]` packs as many records from
   * index `start` as fit one frame, so the client pages until it has all `count`.
   */
  GetFeatures: 0xd0,
  /**
   * FEATURES 0xD1 — switch one feature on or off. Req `[id, 0|1]`; an unknown id, an
   * attempt to disable an always-on feature, or a non-boolean value answers BadArg.
   * Applied live, persisted by the next CONFIG.SAVE.
   */
  SetFeatureEnabled: 0xd1,

  // --- SYSTEM (0xFx) ---
  // The reset ops (0xF0/0xF1) reset the MCU and never reply (the device resets
  // before it could); the host fires them and treats the USB disconnect as the
  // acknowledgement. The USB-personality ops select / read the device's USB mode.
  /** SYSTEM 0xF0 — reset into the wb32-dfu bootloader. No reply (device resets). */
  SystemEnterDfu: 0xf0,
  /** SYSTEM 0xF1 — reboot the firmware. No reply (device resets). */
  SystemReboot: 0xf1,
  /**
   * SYSTEM 0xF2 — select the USB personality. Req `[mode]` (0 = normal, 1 = MIDI,
   * 2 = XInput). A change re-enumerates the device, so it is acknowledged by the
   * USB disconnect like a reset (fire-and-forget); re-selecting the current mode
   * replies normally. An out-of-range mode is BadArg.
   */
  SystemSetUsbMode: 0xf2,
  /** SYSTEM 0xF3 — get the current USB personality. Reply `[mode]`. */
  SystemGetUsbMode: 0xf3,
  /**
   * SYSTEM 0xF4 — set the HID digitizer's absolute pointer (host/test control).
   * Req `[flags, x_lo, x_hi, y_lo, y_hi]`: flags bit0 = tip, bit1 = in-range;
   * X/Y unsigned LE over 0..=32767.
   */
  SystemSetDigitizer: 0xf4,
} as const;
