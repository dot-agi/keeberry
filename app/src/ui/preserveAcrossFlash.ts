// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * The "Preserve keymap & settings across firmware flashes" preference, persisted
 * in `localStorage` and defaulting to **on**. It gates the persist-across-flash
 * flow: when on, the DFU action backs the config up first and a (re)connect
 * restores a compatible backup. Stored as a single flag so the DFU panel (UI) and
 * the connection lifecycle (which reads it fresh at connect time) share one
 * source of truth.
 */
import { useCallback, useState } from 'react';

/** `localStorage` key for the preserve-across-flash flag. */
const STORAGE_KEY = 'keeberry.preserveAcrossFlash';

/**
 * Read the preference, defaulting to `true` when unset or unreadable (privacy
 * mode, disabled storage): the safe default is to preserve settings.
 */
export function readPreserveAcrossFlash(): boolean {
  try {
    // Absent → on by default; only an explicit 'false' turns it off.
    return localStorage.getItem(STORAGE_KEY) !== 'false';
  } catch {
    return true;
  }
}

/** Persist the preference (best-effort; ignores storage being unavailable). */
export function writePreserveAcrossFlash(enabled: boolean): void {
  try {
    localStorage.setItem(STORAGE_KEY, enabled ? 'true' : 'false');
  } catch {
    // Storage unavailable (privacy mode / quota) — the in-memory value still drives this session.
  }
}

/** React state bound to the persisted preference: `[enabled, setEnabled]`. */
export function usePreserveAcrossFlash(): [boolean, (enabled: boolean) => void] {
  const [enabled, setEnabled] = useState(readPreserveAcrossFlash);
  const update = useCallback((next: boolean) => {
    writePreserveAcrossFlash(next);
    setEnabled(next);
  }, []);
  return [enabled, update];
}
