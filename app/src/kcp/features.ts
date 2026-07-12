// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * FEATURES group (0xDx) wire helpers. The firmware exposes one registry-owned
 * dispatcher that enumerates every registered feature and toggles its runtime master
 * switch, so the GUI renders a generic toggle list with no per-feature code. Mirrors
 * `features::features_dispatch` in `firmware/src/features/mod.rs` byte-for-byte.
 *
 * Ops (low nibble of CMD):
 *  - GET_FEATURES (0xD0): request `[start]`; reply `[count, page_len, {id, enabled,
 *    name_len, name_bytes}…]`. The first byte is the total feature count, the second
 *    the number of records packed in this page (the firmware fits as many from `start`
 *    as one 29-byte payload holds), then the records. The client pages by record count
 *    until it has all `count`.
 *  - SET_FEATURE_ENABLED (0xD1): request `[id, 0|1]`. An unknown id, an attempt to
 *    disable an always-on (structural) feature, or a non-boolean value answers BadArg.
 *    Applied live and persisted in the config blob (schema v10).
 */

/** One feature's record as reported by GET_FEATURES. */
export interface FeatureRecord {
  /** Stable feature id (the `FeatureId` discriminant); keys host state, not position. */
  id: number;
  /** Whether the feature is currently switched on. */
  enabled: boolean;
  /** Human label for the toggle. */
  name: string;
}

/** One decoded GET_FEATURES page: the total count plus the records it carried. */
export interface FeaturesPage {
  /** Total number of registered features (the same in every page). */
  count: number;
  /** The feature records packed in this page. */
  records: FeatureRecord[];
}

const NAME_DECODER = new TextDecoder();

/**
 * Parse a GET_FEATURES reply payload: `[count, page_len, {id, enabled, name_len,
 * name…}…]`. Reads exactly `page_len` records, each a 3-byte header plus its name, so
 * a partial trailing record (a malformed frame) is ignored rather than misread.
 */
export function parseFeaturesPage(payload: Uint8Array): FeaturesPage {
  const count = payload[0];
  const pageLen = payload[1];
  const records: FeatureRecord[] = [];
  let pos = 2;
  for (let i = 0; i < pageLen; i += 1) {
    if (pos + 3 > payload.length) break;
    const id = payload[pos];
    const enabled = payload[pos + 1] !== 0;
    const nameLen = payload[pos + 2];
    if (pos + 3 + nameLen > payload.length) break;
    const name = NAME_DECODER.decode(payload.subarray(pos + 3, pos + 3 + nameLen));
    records.push({ id, enabled, name });
    pos += 3 + nameLen;
  }
  return { count, records };
}

/** Build the GET_FEATURES request payload `[start]` (the first record index to pack). */
export function encodeGetFeaturesArgs(start: number): number[] {
  return [start & 0xff];
}

/** Build the SET_FEATURE_ENABLED request payload `[id, 0|1]`. */
export function encodeSetFeatureEnabledArgs(id: number, enabled: boolean): number[] {
  return [id & 0xff, enabled ? 1 : 0];
}
