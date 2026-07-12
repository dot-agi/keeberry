// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Config backup envelope: the on-disk / `localStorage` wrapper around a
 * {@link FullConfig}, stamped with a keeberry magic and the device's config
 * schema version. The schema version is the compatibility key — the firmware does
 * an **exact-match** check on its flash blob (`config.rs`), never a migration, so
 * a backup is restorable only into firmware reporting the same schema version.
 *
 * One envelope shape serves both features that move a config off the device: the
 * downloadable export/import file, and the persist-across-flash `pendingRestore`
 * record. The compatibility decisions live here as pure functions so the
 * version-mismatch refusal (import) and reset-detection (flash restore) are
 * unit-testable without a device or the DOM.
 */
import type { DeviceInfo } from './info';
import type { FullConfig } from './snapshot';

/** Magic identifying a keeberry config backup; rejects unrelated JSON on import. */
export const KEEBERRY_BACKUP_MAGIC = 'keeberry-config-backup';

/** A config backup: the magic, the device's schema version, context, and the values. */
export interface ConfigBackup {
  /** Always {@link KEEBERRY_BACKUP_MAGIC}; the file-type check. */
  magic: typeof KEEBERRY_BACKUP_MAGIC;
  /**
   * The device config schema version this backup was read from
   * (`DeviceInfo.schemaVersion`). The persist-across-flash compatibility key.
   */
  schemaVersion: number;
  /** Chip id the backup came from (`WB32FQ95`); informational. */
  chip: string;
  /** Firmware version the backup came from (`major.minor.patch`); informational. */
  firmwareVersion: string;
  /** ISO-8601 timestamp the backup was taken; informational. */
  savedAt: string;
  /** The complete editable device state. */
  config: FullConfig;
}

/**
 * Build a backup envelope from a device descriptor and a read config. `savedAt`
 * defaults to now; tests pass a fixed value. The schema version and device
 * context come straight from the INFO descriptor.
 */
export function buildBackup(
  deviceInfo: DeviceInfo,
  config: FullConfig,
  savedAt: string = new Date().toISOString(),
): ConfigBackup {
  return {
    magic: KEEBERRY_BACKUP_MAGIC,
    schemaVersion: deviceInfo.schemaVersion,
    chip: deviceInfo.chip,
    firmwareVersion: deviceInfo.firmwareVersionString,
    savedAt,
    config,
  };
}

/** Serialise a backup to pretty-printed JSON for a download / `localStorage`. */
export function serializeBackup(backup: ConfigBackup): string {
  return JSON.stringify(backup, null, 2);
}

/**
 * Parse and validate backup JSON. Throws a clear {@link Error} when the text is
 * not JSON, lacks the keeberry magic, or is missing the schema version / config —
 * so a stray file or a corrupt `localStorage` record is rejected, never silently
 * mis-applied.
 */
export function parseBackup(text: string): ConfigBackup {
  let parsed: unknown;
  try {
    parsed = JSON.parse(text);
  } catch {
    throw new Error('This file is not valid JSON, so it is not a keeberry config backup.');
  }
  if (typeof parsed !== 'object' || parsed === null) {
    throw new Error('This file is not a keeberry config backup.');
  }
  const record = parsed as Record<string, unknown>;
  if (record.magic !== KEEBERRY_BACKUP_MAGIC) {
    throw new Error('This file is not a keeberry config backup (missing keeberry marker).');
  }
  if (typeof record.schemaVersion !== 'number' || !Number.isFinite(record.schemaVersion)) {
    throw new Error('This keeberry backup is missing its config schema version.');
  }
  if (typeof record.config !== 'object' || record.config === null) {
    throw new Error('This keeberry backup is missing its settings.');
  }
  return parsed as ConfigBackup;
}

/** Whether a backup's schema version matches what the connected firmware reports. */
function schemaMatches(backup: ConfigBackup, deviceInfo: DeviceInfo): boolean {
  return backup.schemaVersion === deviceInfo.schemaVersion;
}

/** The outcome of checking a backup against the connected device. */
export type CompatibilityResult = { ok: true } | { ok: false; message: string };

/**
 * Decide whether an imported backup may be written to the connected device.
 * Compatible only when the schema versions match exactly; otherwise refuses with a
 * message naming both versions, since writing a different-schema config could
 * corrupt settings.
 */
export function checkImportCompatibility(
  backup: ConfigBackup,
  deviceInfo: DeviceInfo,
): CompatibilityResult {
  if (schemaMatches(backup, deviceInfo)) {
    return { ok: true };
  }
  return {
    ok: false,
    message:
      `This backup was saved from config schema v${backup.schemaVersion}, but the ` +
      `connected keyboard uses schema v${deviceInfo.schemaVersion}. Importing could ` +
      `corrupt settings, so it was refused.`,
  };
}

/** The plan for a pending backup after a (re)connect: restore it, or skip with a reason. */
export type RestorePlan = { action: 'restore' } | { action: 'skip'; message: string };

/**
 * Decide what to do with a persist-across-flash `pendingRestore` backup once a
 * device (re)connects. Same-schema → restore. Different-schema means the new
 * firmware changed the config format and reset to defaults, so the backup is *not*
 * applied (it would not fit) but is kept so the user can still export it; the
 * message names both versions (the build-level disable).
 */
export function planFlashRestore(backup: ConfigBackup, deviceInfo: DeviceInfo): RestorePlan {
  if (schemaMatches(backup, deviceInfo)) {
    return { action: 'restore' };
  }
  return {
    action: 'skip',
    message:
      `New firmware changed the config format (schema v${backup.schemaVersion}→` +
      `v${deviceInfo.schemaVersion}); settings were reset to defaults — your backup is ` +
      `still available to export.`,
  };
}
