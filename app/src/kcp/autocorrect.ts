// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * TEXT group (0x9x) wire helpers. Autocorrect: a rolling word buffer matched
 * against a compiled-in typo→correction dictionary; on a whole-word match the
 * firmware injects backspaces + the correction. Mirrors `kcp.rs`'s autocorrect
 * dispatch byte-for-byte.
 *
 * Ops (low nibble of CMD):
 *  - AUTOCORRECT_INFO (0x90): no request payload; reply `[enabled, count_lo,
 *    count_hi]` — the enable flag and the compiled-in dictionary entry count
 *    (little-endian u16).
 *  - AUTOCORRECT_SET (0x91): request `[0|1]`; any other value answers BadArg.
 *    An alias for the FEATURES group's toggle on autocorrect's bit — applied live and
 *    persisted in the config blob's feature-enable bitmap (schema v10).
 */
import { readU16LE } from './bytes';

/** The autocorrect state an AUTOCORRECT_INFO reply carries. */
export interface AutocorrectInfo {
  /** Whether autocorrect is enabled. */
  enabled: boolean;
  /** Number of typo→correction pairs compiled into the firmware dictionary. */
  entryCount: number;
}

/** Parse an AUTOCORRECT_INFO reply: `[enabled, count(2 LE)]`. */
export function parseAutocorrectInfo(payload: Uint8Array): AutocorrectInfo {
  return { enabled: payload[0] !== 0, entryCount: readU16LE(payload, 1) };
}

/** Build the AUTOCORRECT_SET request payload `[0|1]`. */
export function encodeSetAutocorrectArgs(enabled: boolean): number[] {
  return [enabled ? 1 : 0];
}
