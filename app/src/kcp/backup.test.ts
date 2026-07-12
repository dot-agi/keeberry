// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import {
  KEEBERRY_BACKUP_MAGIC,
  buildBackup,
  checkImportCompatibility,
  parseBackup,
  planFlashRestore,
  serializeBackup,
  type ConfigBackup,
} from './backup';
import type { DeviceInfo } from './info';
import type { FullConfig } from './snapshot';

/** A minimal but complete device descriptor reporting config schema `version`. */
function deviceInfo(version: number): DeviceInfo {
  return {
    firmwareVersion: { major: 0, minor: 1, patch: 0 },
    firmwareVersionString: '0.1.0',
    chip: 'WB32FQ95',
    rows: 6,
    cols: 15,
    layers: 4,
    connection: { code: 0, label: 'USB' },
    schemaVersion: version,
  };
}

/** A tiny, well-formed config (the envelope is opaque to the values). */
function sampleConfig(): FullConfig {
  return {
    keymap: [[[4]]],
    nkro: true,
    layerConfig: { defaultLayer: 0, triEnabled: false, triL1: 0, triL2: 0, triL3: 0 },
    debounce: { algorithm: 0, interval: 5 },
    tuning: {
      autoShiftEnabled: false,
      autoShiftTimeoutMs: 175,
      leaderTimeoutMs: 300,
      tapHoldTermMs: 200,
      permissiveHold: false,
      holdOnOtherKeyPress: false,
      retroTapping: false,
      chordalHold: false,
      quickTapTermMs: 200,
    },
    rgb: {
      mode: 1,
      hue: 10,
      sat: 200,
      val: 150,
      brightness: 84,
      enabled: false,
      speed: 64,
      indicators: true,
    },
    zones: [],
    socd: [{ a: 4, b: 7, mode: 1 }],
    overrides: [null],
    tapDance: [null],
    combos: [null],
    leaders: [null],
    macros: [[]],
    features: [{ id: 0, enabled: true, name: 'SOCD Cleanup' }],
  };
}

describe('buildBackup', () => {
  it('stamps the magic, the device schema version and context, and the config', () => {
    const config = sampleConfig();
    const backup = buildBackup(deviceInfo(2), config, '2026-01-01T00:00:00.000Z');
    expect(backup).toEqual({
      magic: KEEBERRY_BACKUP_MAGIC,
      schemaVersion: 2,
      chip: 'WB32FQ95',
      firmwareVersion: '0.1.0',
      savedAt: '2026-01-01T00:00:00.000Z',
      config,
    });
  });

  it('defaults savedAt to an ISO timestamp', () => {
    const backup = buildBackup(deviceInfo(2), sampleConfig());
    expect(Number.isNaN(Date.parse(backup.savedAt))).toBe(false);
  });
});

describe('serializeBackup / parseBackup', () => {
  it('round-trips a backup through JSON', () => {
    const backup = buildBackup(deviceInfo(2), sampleConfig(), '2026-01-01T00:00:00.000Z');
    expect(parseBackup(serializeBackup(backup))).toEqual(backup);
  });

  it('rejects text that is not JSON', () => {
    expect(() => parseBackup('not json{')).toThrow(/not valid JSON/);
  });

  it('rejects JSON that is not a keeberry backup (bad magic)', () => {
    expect(() => parseBackup(JSON.stringify({ magic: 'something-else' }))).toThrow(
      /not a keeberry config backup/,
    );
  });

  it('rejects a backup missing its schema version', () => {
    const text = JSON.stringify({ magic: KEEBERRY_BACKUP_MAGIC, config: {} });
    expect(() => parseBackup(text)).toThrow(/schema version/);
  });

  it('rejects a backup missing its config', () => {
    const text = JSON.stringify({ magic: KEEBERRY_BACKUP_MAGIC, schemaVersion: 2 });
    expect(() => parseBackup(text)).toThrow(/missing its settings/);
  });
});

describe('checkImportCompatibility', () => {
  it('accepts a backup whose schema matches the connected device', () => {
    const backup: ConfigBackup = buildBackup(deviceInfo(2), sampleConfig());
    expect(checkImportCompatibility(backup, deviceInfo(2))).toEqual({ ok: true });
  });

  it('refuses a schema mismatch with a message naming both versions', () => {
    const backup: ConfigBackup = buildBackup(deviceInfo(1), sampleConfig());
    const result = checkImportCompatibility(backup, deviceInfo(2));
    expect(result.ok).toBe(false);
    if (!result.ok) {
      expect(result.message).toContain('v1');
      expect(result.message).toContain('v2');
      expect(result.message).toMatch(/refused/);
    }
  });
});

describe('planFlashRestore', () => {
  it('restores when the schema matches', () => {
    const backup = buildBackup(deviceInfo(2), sampleConfig());
    expect(planFlashRestore(backup, deviceInfo(2))).toEqual({ action: 'restore' });
  });

  it('skips a schema mismatch, naming both versions and keeping the backup exportable', () => {
    const backup = buildBackup(deviceInfo(1), sampleConfig());
    const plan = planFlashRestore(backup, deviceInfo(3));
    expect(plan.action).toBe('skip');
    if (plan.action === 'skip') {
      expect(plan.message).toContain('v1→v3');
      expect(plan.message).toMatch(/reset to defaults/);
      expect(plan.message).toMatch(/still available to export/);
    }
  });
});
