// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * The persist-across-flash `pendingRestore` record: a {@link ConfigBackup} stashed
 * in `localStorage` just before entering DFU, and consumed when a device next
 * (re)connects. It survives the page across the flash (the user re-plugs / the
 * device re-enumerates), which a flash-scoped value could not. Kept apart from the
 * download export only by where it lives — the envelope is identical.
 */
import { type ConfigBackup, parseBackup, serializeBackup } from '../kcp';

/** `localStorage` key for the pending post-flash restore. */
const STORAGE_KEY = 'keeberry.pendingRestore';

/**
 * Read the pending restore, or `null` when there is none or it is unreadable /
 * corrupt. Validates through {@link parseBackup}, so a malformed record is
 * treated as absent rather than mis-applied.
 */
export function readPendingRestore(): ConfigBackup | null {
  try {
    const text = localStorage.getItem(STORAGE_KEY);
    return text === null ? null : parseBackup(text);
  } catch {
    return null;
  }
}

/**
 * Stash a backup to restore after the next (re)connect. Returns whether it was
 * written; `false` (storage unavailable / over quota) lets the DFU caller refuse
 * to proceed rather than silently lose the user's settings.
 */
export function writePendingRestore(backup: ConfigBackup): boolean {
  try {
    localStorage.setItem(STORAGE_KEY, serializeBackup(backup));
    return true;
  } catch {
    return false;
  }
}

/** Clear the pending restore (after it is applied, or when no longer wanted). */
export function clearPendingRestore(): void {
  try {
    localStorage.removeItem(STORAGE_KEY);
  } catch {
    // Nothing to do if storage is unavailable.
  }
}
