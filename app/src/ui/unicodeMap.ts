// SPDX-License-Identifier: GPL-2.0-or-later
import { UNICODE_MAP_SLOTS, type UnicodeInfo } from '../kcp';

/**
 * The Unicode codepoint map lives in device RAM only (no flash persistence), so the host
 * owns the authoritative copy and re-uploads it on every connect. This module holds that
 * cache plus the restore both the connection lifecycle and the Unicode panel drive, so a
 * just-powered keyboard matches what the host holds regardless of which panel is open.
 */

/** Where the codepoint map is cached between connects (the device holds it in RAM only). */
const STORAGE_KEY = 'keeberry.unicode.map.v1';

/** The subset of the kcp client the map restore needs. */
interface UnicodeMapClient {
  unicodeGet(): Promise<UnicodeInfo>;
  unicodeSetMap(slot: number, codepoint: number): Promise<void>;
}

/** Whether `cp` is a Unicode scalar value (mirror of the firmware's `is_scalar`). */
export function isScalar(cp: number): boolean {
  return Number.isInteger(cp) && cp >= 0 && cp <= 0x10ffff && !(cp >= 0xd800 && cp <= 0xdfff);
}

/** Read the cached map as a fixed-length codepoint array, tolerating missing/corrupt storage. */
export function readStoredMap(): number[] {
  const map = new Array<number>(UNICODE_MAP_SLOTS).fill(0);
  if (typeof window === 'undefined') return map;
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY);
    if (!raw) return map;
    const parsed: unknown = JSON.parse(raw);
    if (Array.isArray(parsed)) {
      for (let slot = 0; slot < UNICODE_MAP_SLOTS; slot += 1) {
        const cp = parsed[slot];
        if (typeof cp === 'number' && isScalar(cp)) map[slot] = cp;
      }
    }
  } catch {
    // A corrupt cache just yields an empty map; the user can re-enter the slots.
  }
  return map;
}

/** Persist the codepoint map so it survives a disconnect and is re-uploaded next connect. */
export function writeStoredMap(map: number[]): void {
  if (typeof window === 'undefined') return;
  try {
    window.localStorage.setItem(STORAGE_KEY, JSON.stringify(map));
  } catch {
    // The map still applies live this session; a storage failure only skips the cache.
  }
}

/**
 * Re-upload a codepoint map to the device over SET_MAP, one slot at a time. The map is
 * RAM-only on the device (cleared on power-cycle), so this restores it after every
 * (re)connect — independent of whether the Unicode panel is mounted. The slot count comes
 * from the device's own GET, which is also returned so a caller can reuse it (the panel reads
 * the active mode and slot count from it). Defaults to the locally cached map.
 */
export async function restoreUnicodeMap(
  client: UnicodeMapClient,
  map: number[] = readStoredMap(),
): Promise<UnicodeInfo> {
  const info = await client.unicodeGet();
  const count = Math.min(info.slots, UNICODE_MAP_SLOTS);
  for (let slot = 0; slot < count; slot += 1) {
    await client.unicodeSetMap(slot, map[slot] ?? 0);
  }
  return info;
}
