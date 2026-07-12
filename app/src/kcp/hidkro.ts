// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * HID_KRO group (0x3x) wire helpers. Rollover mode: boot 6-key rollover (the
 * default) vs full N-key rollover, held live in the firmware and read by the
 * report loop each scan. Mirrors `kcp.rs`'s `hid_kro_dispatch` byte-for-byte.
 *
 * Ops (low nibble of CMD):
 *  - GET_KRO (0x30): no request payload; reply `[nkro_enabled]` (1 = NKRO on,
 *    0 = boot 6KRO).
 *  - SET_KRO (0x31): request `[0|1]` (0 = boot 6KRO, 1 = NKRO); any other value
 *    answers BadArg. Applied live on the next scan.
 */

/** Parse a GET_KRO reply payload: NKRO enabled when byte 0 is non-zero. */
export function parseKro(payload: Uint8Array): boolean {
  return payload[0] !== 0;
}

/** Build the SET_KRO request payload `[0|1]`. */
export function encodeSetKroArgs(nkroEnabled: boolean): number[] {
  return [nkroEnabled ? 1 : 0];
}
