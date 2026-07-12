// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Browser download of a {@link ConfigBackup} as a JSON file. Shared by the CONFIG
 * panel's "Export to file" (a backup read live off the device) and the
 * persist-across-flash "Export pre-flash backup" (the retained `pendingRestore`
 * record), so both emit an identically named, identically formatted file from the
 * one {@link serializeBackup} envelope rather than reinventing the download twice.
 */
import { serializeBackup, type ConfigBackup } from '../kcp';

/**
 * Trigger a browser download of `backup` as pretty-printed JSON. The filename
 * carries the config schema version and a filesystem-safe `savedAt` stamp, so an
 * exported file self-describes the schema it can be re-imported into.
 */
export function downloadBackup(backup: ConfigBackup): void {
  const stamp = backup.savedAt.replace(/[:.]/g, '-');
  const filename = `keeberry-config-schema-v${backup.schemaVersion}-${stamp}.json`;
  const blob = new Blob([serializeBackup(backup)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const anchor = document.createElement('a');
  anchor.href = url;
  anchor.download = filename;
  anchor.click();
  URL.revokeObjectURL(url);
}
