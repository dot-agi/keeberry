// SPDX-License-Identifier: GPL-2.0-or-later
import { useEffect, useRef, useState } from 'react';
import {
  buildBackup,
  checkImportCompatibility,
  parseBackup,
  readFullConfig,
  writeFullConfig,
  type DeviceInfo,
  type KcpClient,
  type StorageInfo,
} from '../kcp';
import { downloadBackup } from './downloadBackup';
import {
  ActionMenu,
  ErrorBanner,
  Field,
  InfoHint,
  Panel,
  StatusBanner,
  UnsavedBadge,
} from './Panel';
import { friendlyPanelError } from './panelError';
import { useConfigSave } from './useConfigSave';
import { useUnsavedChanges } from './useUnsavedChanges';

interface ConfigPanelProps {
  client: KcpClient;
  deviceInfo: DeviceInfo;
}

type Busy = 'defaults' | 'export' | 'import' | null;

/**
 * Persistence control for the CONFIG group. `CONFIG.SAVE` now persists the
 * **complete** device state (keymap, NKRO, RGB, the behaviour tables and macros),
 * so this panel:
 *  - saves everything to flash and resets to defaults (CONFIG_SAVE / LOAD_DEFAULTS);
 *  - shows an unsaved-changes badge while any live edit is unpersisted;
 *  - exports the full config + schema version to a JSON file and imports one back,
 *    refusing a backup whose schema version does not match this firmware.
 *
 * The live runtime tunables (debounce, auto-shift, leader) live in the Tuning panel.
 */
export function ConfigPanel({ client, deviceInfo }: ConfigPanelProps) {
  const [storage, setStorage] = useState<StorageInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);
  const unsaved = useUnsavedChanges(client);
  const {
    saving,
    status: saveStatus,
    error: saveError,
    save: persistConfig,
    clearFeedback: clearSaveFeedback,
  } = useConfigSave(client);
  // Save owns its own busy flag; the file/defaults actions share `busy`. Either
  // blocks the others, and each action clears every banner first so only its own
  // outcome shows.
  const anyBusy = busy !== null || saving;

  function resetFeedback() {
    setError(null);
    setStatus(null);
    clearSaveFeedback();
  }

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        const info = await client.getStorageInfo();
        if (!cancelled) setStorage(info);
      } catch (err) {
        if (!cancelled) setError(friendlyPanelError(err));
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [client]);

  async function save() {
    resetFeedback();
    if (!(await persistConfig())) return;
    try {
      setStorage(await client.getStorageInfo());
    } catch {
      // The save itself succeeded, so keep its success banner: a failed storage
      // refresh just leaves the previous reading rather than masking the save.
    }
  }

  async function loadDefaults() {
    if (
      !window.confirm(
        'Reset every setting to factory defaults? This replaces the current keymap, lighting, behaviors, and macros. Save afterward to keep the reset.',
      )
    ) {
      return;
    }
    setBusy('defaults');
    resetFeedback();
    try {
      await client.configLoadDefaults();
      setStatus('Loaded factory defaults. Save all settings to keep them.');
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(null);
    }
  }

  async function exportToFile() {
    setBusy('export');
    resetFeedback();
    try {
      const config = await readFullConfig(client, deviceInfo);
      downloadBackup(buildBackup(deviceInfo, config));
      setStatus('Exported all settings to a file.');
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(null);
    }
  }

  async function importFromFile(file: File) {
    setBusy('import');
    resetFeedback();
    try {
      const backup = parseBackup(await file.text());
      const compat = checkImportCompatibility(backup, deviceInfo);
      if (!compat.ok) {
        setError('This settings file is not compatible with this keyboard.');
        return;
      }
      await writeFullConfig(client, backup.config);
      setStatus('Imported all settings. Save all settings to keep them on the keyboard.');
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(null);
    }
  }

  return (
    <Panel
      title="Settings"
      description={
        <>
          Keymap, lighting, behavior, and macro edits apply immediately. Save keeps the complete
          setup after restart.
        </>
      }
      headerExtra={unsaved ? <UnsavedBadge /> : null}
    >
      {(error ?? saveError) && <ErrorBanner>{error ?? saveError}</ErrorBanner>}
      {(status ?? saveStatus) && <StatusBanner>{status ?? saveStatus}</StatusBanner>}

      <div className="mb-5 flex flex-wrap items-center gap-2">
        <button
          type="button"
          disabled={anyBusy}
          onClick={() => void save()}
          className="kb-control kb-control-primary"
        >
          {saving ? 'Saving…' : 'Save settings'}
        </button>
        <ActionMenu
          label="More"
          align="left"
          actions={[
            {
              id: 'load-defaults',
              label: busy === 'defaults' ? 'Loading defaults' : 'Load defaults',
              detail: 'Replace current setup with defaults',
              tone: 'warn',
              disabled: anyBusy,
              onSelect: () => void loadDefaults(),
            },
            {
              id: 'export-file',
              label: busy === 'export' ? 'Exporting' : 'Export file',
              detail: 'Save a settings file',
              disabled: anyBusy,
              onSelect: () => void exportToFile(),
            },
            {
              id: 'import-file',
              label: busy === 'import' ? 'Importing' : 'Import file',
              detail: 'Load a settings file',
              disabled: anyBusy,
              onSelect: () => fileInputRef.current?.click(),
            },
          ]}
        />
        <InfoHint label="Backup file details">
          Settings files keep your keyboard setup portable between compatible boards.
        </InfoHint>
        <input
          ref={fileInputRef}
          type="file"
          accept="application/json,.json"
          className="hidden"
          onChange={(event) => {
            const file = event.target.files?.[0];
            // Reset so re-selecting the same file fires change again.
            event.target.value = '';
            if (file) void importFromFile(file);
          }}
        />
      </div>

      {storage ? (
        <dl className="kb-field-grid sm:grid-cols-4">
          <Field label="Saved setup" value={storage.valid ? 'available' : 'none'} />
          <Field
            label="Keyboard slot"
            value={storage.valid ? 'ready' : 'empty'}
            tone={storage.valid ? 'ok' : 'muted'}
          />
        </dl>
      ) : (
        !error && <p className="text-sm text-slate-400">Checking saved setup…</p>
      )}
    </Panel>
  );
}
