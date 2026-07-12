// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * WIRELESS group (0x8x) wire helpers. The state layout mirrors `kcp.rs`'s
 * `pack_wireless_state` byte-for-byte and the `devs` byte is the firmware's
 * `wireless::Devs` code (`mod.rs`). The connection-state byte is the radio's
 * `MdState` code (`wireless/md.rs`).
 *
 * Ops (low nibble of CMD):
 *  - GET_STATE (0x80): no request payload; reply `[devs, state, battery, version]`.
 *  - SET_MODE (0x81): request `[devs]`; an unknown code answers BadArg.
 *  - PAIR (0x82): no payload — (re)pair the current transport (reset = true).
 *  - UNPAIR (0x83): no payload — clear the active channel's bond.
 *  - SET_SLEEP_POLICY (0x84): request `[enable_bt, enable_2g4]` (0 = off).
 *  - GET_BATTERY (0x85): reply `[battery]`; also triggers a fresh measurement.
 */

/**
 * Output-transport codes (`wireless::Devs::code` in `mod.rs`): USB = 0,
 * BT1..BT3 = 1..3, 2.4 GHz = 6. These are the bytes WLS_SET_MODE carries and the
 * `devs` byte WLS_GET_STATE returns; the firmware's `Devs::from_u8` accepts only
 * these (BT4/BT5 and any other value answer BadArg).
 */
export const Devs = {
  Usb: 0,
  Bt1: 1,
  Bt2: 2,
  Bt3: 3,
  G2_4: 6,
} as const;

export type DevsCode = (typeof Devs)[keyof typeof Devs];

/**
 * The host-selectable output transports, in UI order, with their `Devs` codes. USB
 * is intentionally absent: it is the cable-auto-selected top priority, not a mode the
 * host picks — selecting it on battery would route HID to a link with no host. The
 * firmware rejects `SET_MODE(Usb)` for the same reason; plugging a cable in is how you
 * reach USB.
 */
export const WIRELESS_MODES: readonly { code: DevsCode; label: string }[] = [
  { code: Devs.G2_4, label: '2.4 GHz' },
  { code: Devs.Bt1, label: 'Bluetooth 1' },
  { code: Devs.Bt2, label: 'Bluetooth 2' },
  { code: Devs.Bt3, label: 'Bluetooth 3' },
];

/**
 * Radio connection-state codes (`MdState::as_u8` / `MD_STATE_*` in
 * `wireless/md.rs`): NONE = 0, PAIRING = 1, CONNECTED = 2, DISCONNECTED = 3,
 * REJECT = 4.
 */
export const ConnectionState = {
  None: 0,
  Pairing: 1,
  Connected: 2,
  Disconnected: 3,
  Reject: 4,
} as const;

export type ConnectionStateCode = (typeof ConnectionState)[keyof typeof ConnectionState];

const CONNECTION_STATE_LABELS: Record<number, string> = {
  [ConnectionState.None]: 'Idle',
  [ConnectionState.Pairing]: 'Pairing',
  [ConnectionState.Connected]: 'Connected',
  [ConnectionState.Disconnected]: 'Disconnected',
  [ConnectionState.Reject]: 'Rejected',
};

/** Human-readable label for a radio connection-state byte (handles unknowns). */
export function connectionStateLabel(code: number): string {
  return CONNECTION_STATE_LABELS[code] ?? 'Unknown';
}

/**
 * A decoded wireless link snapshot (`pack_wireless_state`), payload-relative
 * offsets: `0` transport (`Devs`), `1` connection state (`MdState`), `2` battery
 * percent, `3` radio firmware version.
 */
export interface WirelessState {
  /** Active output transport (`wireless::Devs` code). */
  devs: number;
  /** Radio connection state (`MdState` code). */
  state: number;
  /** Battery level, percent (`md_info.bat`). */
  battery: number;
  /** Radio firmware version (`md_info.version`). */
  version: number;
}

/** Parse a WLS_GET_STATE reply payload into a {@link WirelessState}. */
export function parseWirelessState(payload: Uint8Array): WirelessState {
  return {
    devs: payload[0],
    state: payload[1],
    battery: payload[2],
    version: payload[3],
  };
}

/** Parse a WLS_GET_BATTERY reply payload: the battery percent in byte 0. */
export function parseBattery(payload: Uint8Array): number {
  return payload[0];
}

/** Build the WLS_SET_SLEEP_POLICY request payload `[enable_bt, enable_2g4]`. */
export function encodeSleepPolicyArgs(enableBt: boolean, enable2g4: boolean): number[] {
  return [enableBt ? 1 : 0, enable2g4 ? 1 : 0];
}
