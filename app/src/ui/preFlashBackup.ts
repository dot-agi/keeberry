// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * The shared pre-flash backup step, used by BOTH paths that enter the bootloader:
 * the manual "Enter bootloader" in {@link SystemPanel} and the one-click "Update
 * firmware" in {@link useFirmwareFlash}. Entering DFU resets the MCU and drops the
 * device off USB, so the complete config must be stashed as the `pendingRestore`
 * record *before* the reset — a (re)connect then restores it (see
 * `useKcpDevice`). Centralising it here is what keeps the two entry paths
 * identical: a flash preserves settings the same way however it was started.
 */
import {
  buildBackup,
  readFullConfig,
  type ConfigBackup,
  type DeviceInfo,
  type KcpClient,
} from '../kcp';
import { clearPendingRestore, writePendingRestore } from './pendingRestore';

/**
 * Outcome of the pre-flash backup. `ok: false` means the backup could not be
 * taken, so the caller must ABORT the flash rather than reset the device to
 * defaults while the preserve toggle reads "On"; `message` is user-facing.
 * `ok: true` carries the stashed backup, or `null` when preserve was off.
 */
export type PreFlashBackupResult =
  | { ok: true; backup: ConfigBackup | null }
  | { ok: false; message: string };

/**
 * Stash (or, with the toggle off, intentionally drop) the pre-flash config backup.
 *
 * With `preserve` on, read the full live config, build the envelope and write it
 * to `pendingRestore`; a read or storage failure returns `ok: false` so the flash
 * is never started after telling the user their settings are safe. With it off,
 * clear any stale backup so a later reconnect will not restore it.
 */
export async function backupBeforeFlash(
  client: KcpClient,
  deviceInfo: DeviceInfo,
  preserve: boolean,
): Promise<PreFlashBackupResult> {
  if (!preserve) {
    clearPendingRestore();
    return { ok: true, backup: null };
  }
  let backup: ConfigBackup;
  try {
    backup = buildBackup(deviceInfo, await readFullConfig(client, deviceInfo));
  } catch (err) {
    const reason = err instanceof Error ? err.message : String(err);
    return {
      ok: false,
      message: `Could not back up settings before DFU: ${reason} — DFU not started.`,
    };
  }
  if (!writePendingRestore(backup)) {
    return {
      ok: false,
      message:
        'Could not store the settings backup (browser storage unavailable) — DFU not started.',
    };
  }
  return { ok: true, backup };
}
