// SPDX-License-Identifier: GPL-2.0-or-later
import { useState } from 'react';
import { type ConfigBackup, type DeviceInfo, type KcpClient } from '../kcp';
import { downloadBackup } from './downloadBackup';
import { type BundledFirmware } from './nativeFlash';
import { ActionMenu, ErrorBanner, InfoHint, Panel, StatusBanner } from './Panel';
import { friendlyPanelError } from './panelError';
import { usePreserveAcrossFlash } from './preserveAcrossFlash';
import { backupBeforeFlash } from './preFlashBackup';
import { clearPendingRestore, readPendingRestore } from './pendingRestore';

interface SystemPanelProps {
  client: KcpClient;
  deviceInfo: DeviceInfo;
  /** True in the desktop build, where one-click flashing is available. */
  native: boolean;
  /** The firmware version the app bundles (desktop only), or null if unknown. */
  bundledFirmware: BundledFirmware | null;
  /** True while a native flash/reboot is in flight (disables every reset here). */
  flashBusy: boolean;
  /** Start the one-click update: enter DFU, flash the bundled image, reconnect. */
  onUpdateFirmware: () => void;
  /** Reboot a board that is in the bootloader back into its firmware. */
  onRebootToKeyboard: () => void;
}

/**
 * Device-level resets (the SYSTEM group), the firmware version contract, and the
 * persist-across-flash control. "Enter bootloader" resets into the wb32-dfu ROM
 * bootloader for re-flashing; "Reboot" restarts the firmware. Both reset the MCU,
 * so the device drops off USB and this panel unmounts — that disconnect IS the
 * acknowledgement (see `client.enterDfu`). The bootloader action is confirm-
 * guarded since it leaves the board unusable as a keyboard until re-flashed or
 * replugged.
 *
 * In the desktop build two native actions are added: "Update firmware" (the
 * one-click flow — enter DFU, flash the bundled image, reconnect) and "Reboot to
 * keyboard" (leave the bootloader). Both are owned by App (they outlive this
 * panel's unmount-on-disconnect); here they are just buttons. The browser build
 * hides them and notes that flashing is desktop-only — WebHID cannot reach DFU.
 *
 * When "Preserve keymap & settings across firmware flashes" is on (the default),
 * entering DFU first reads the complete config and stashes it as a `pendingRestore`
 * backup; a (re)connect restores it if the new firmware's config schema matches.
 * When the schema differs the restore is skipped and the backup is kept, so this
 * panel surfaces it as a downloadable file (the only flow that exports the retained
 * record rather than the live device), cleared only when the user exports or
 * discards it.
 */
export function SystemPanel({
  client,
  deviceInfo,
  native,
  bundledFirmware,
  flashBusy,
  onUpdateFirmware,
  onRebootToKeyboard,
}: SystemPanelProps) {
  const [busy, setBusy] = useState<'dfu' | 'reboot' | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [status, setStatus] = useState<string | null>(null);
  const [preserve, setPreserve] = usePreserveAcrossFlash();
  // A retained pre-flash backup (a schema-mismatch reconnect keeps it), read once
  // on mount; export/discard are the only things that clear it.
  const [pendingBackup, setPendingBackup] = useState<ConfigBackup | null>(() =>
    readPendingRestore(),
  );

  async function enterDfu() {
    if (
      !window.confirm(
        'Enter recovery mode? The keyboard will disconnect and pause normal typing until it is returned to keyboard mode.',
      )
    ) {
      return;
    }
    setBusy('dfu');
    setError(null);
    setStatus(null);
    try {
      // Stash (or, toggle-off, drop) the pre-flash backup before the reset — the
      // same shared step the one-click "Update firmware" path runs. A failed
      // backup aborts DFU so the user is never told their settings are safe when
      // they are not.
      const preserved = await backupBeforeFlash(client, deviceInfo, preserve);
      if (!preserved.ok) {
        setError(preserved.message);
        return;
      }
      setPendingBackup(preserved.backup);
      await client.enterDfu();
      setStatus(
        preserve
          ? 'Your settings are backed up. Entering recovery mode…'
          : 'Entering recovery mode… the keyboard is disconnecting.',
      );
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(null);
    }
  }

  async function reboot() {
    setBusy('reboot');
    setError(null);
    setStatus(null);
    try {
      await client.reboot();
      setStatus('Restarting… the keyboard will reconnect shortly.');
    } catch (err) {
      setError(friendlyPanelError(err));
    } finally {
      setBusy(null);
    }
  }

  // Download the retained pre-flash backup, then drop it — an explicit user action,
  // so the record is never cleared silently. The record already holds the full
  // config and its original schema, so no device read is needed.
  function exportPendingBackup() {
    if (!pendingBackup) {
      return;
    }
    downloadBackup(pendingBackup);
    clearPendingRestore();
    setPendingBackup(null);
    setError(null);
    setStatus('Exported the restore point to a file.');
  }

  // The explicit "I don't want it" path; confirm-guarded since it permanently drops
  // the only copy of the pre-flash settings.
  function discardPendingBackup() {
    if (!window.confirm('Discard the saved restore point? This permanently removes it.')) {
      return;
    }
    clearPendingRestore();
    setPendingBackup(null);
    setError(null);
    setStatus('Discarded the restore point.');
  }

  return (
    <Panel
      title="Maintenance"
      description="Update, restart, and recovery controls can briefly disconnect the keyboard."
    >
      {error && <ErrorBanner>{error}</ErrorBanner>}
      {status && <StatusBanner>{status}</StatusBanner>}

      <div className="mb-5 grid gap-3 border border-[#1b222a] bg-[#020304] px-3 py-3 lg:grid-cols-[minmax(0,1fr)_auto] lg:items-center">
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-2">
            <span className="min-w-0 break-words text-sm font-medium text-slate-200">
              Preserve keymap &amp; settings
            </span>
            <InfoHint label="Preserve setting details">
              When on, recovery actions save a restore point first and apply it after compatible
              updates.
            </InfoHint>
          </div>
          <span className="mt-1 block font-mono text-[0.65rem] uppercase tracking-wide text-slate-600">
            auto restore
          </span>
        </div>
        <button
          type="button"
          role="switch"
          aria-checked={preserve}
          aria-label="Preserve keymap and settings"
          onClick={() => setPreserve(!preserve)}
          className={[
            'inline-flex shrink-0 items-center gap-2 border px-3 py-1.5 font-mono text-xs font-semibold uppercase transition-colors',
            preserve
              ? 'border-emerald-400/40 bg-emerald-500/15 text-emerald-200'
              : 'border-[#26313c] bg-[#020304] text-slate-400',
          ].join(' ')}
        >
          <span
            className={['h-1.5 w-1.5', preserve ? 'bg-emerald-400' : 'bg-slate-600'].join(' ')}
          />
          {preserve ? 'On' : 'Off'}
        </button>
      </div>

      {pendingBackup && (
        <div className="mb-5 flex items-center justify-between gap-3 border border-sky-500/40 bg-sky-500/10 px-3 py-3 text-xs text-slate-300">
          <div className="min-w-0">
            <div className="flex flex-wrap items-center gap-2">
              <p className="text-sm font-medium text-slate-200">Restore point</p>
              <InfoHint label="Restore point details">
                A saved setup is retained in this browser. Export it to keep a file copy, or discard
                it when it is no longer needed.
              </InfoHint>
            </div>
            <div className="mt-1 flex flex-wrap gap-2 font-mono text-[0.65rem] uppercase tracking-wide text-slate-500">
              <span>{new Date(pendingBackup.savedAt).toLocaleString()}</span>
            </div>
          </div>
          <ActionMenu
            label="Backup"
            align="left"
            actions={[
              {
                id: 'export-pre-flash-backup',
                label: 'Export restore point',
                detail: 'Download then clear retained copy',
                tone: 'ok',
                onSelect: exportPendingBackup,
              },
              {
                id: 'discard-pre-flash-backup',
                label: 'Discard backup',
                detail: 'Permanently clear retained copy',
                tone: 'danger',
                onSelect: discardPendingBackup,
              },
            ]}
          />
        </div>
      )}

      <div className="mb-5 border border-[#1b222a] bg-[#020304] px-3 py-3 text-xs text-slate-400">
        <div className="flex items-center gap-2">
          <p className="text-sm font-medium text-slate-200">Updates</p>
          <InfoHint label="Update details">
            Saved settings can be restored after compatible updates.
          </InfoHint>
        </div>
        <div className="mt-3 grid gap-2 sm:grid-cols-2">
          <div className="border border-[#26313c] bg-[#010203] px-3 py-2">
            <span className="block font-mono text-[0.65rem] uppercase tracking-wide text-slate-600">
              restore
            </span>
            <span className="mt-1 block break-words text-sm text-slate-300">
              Ready for compatible updates
            </span>
          </div>
          {native && bundledFirmware && (
            <div className="border border-[#26313c] bg-[#010203] px-3 py-2">
              <span className="block font-mono text-[0.65rem] uppercase tracking-wide text-slate-600">
                package
              </span>
              <span className="mt-1 block break-words text-sm text-slate-300">Ready</span>
            </div>
          )}
        </div>
      </div>

      <div className="flex flex-wrap items-center gap-2">
        {native && (
          <button
            type="button"
            disabled={busy !== null || flashBusy || !bundledFirmware}
            onClick={onUpdateFirmware}
            className="kb-control kb-control-primary"
          >
            {flashBusy ? 'Updating…' : 'Install update'}
          </button>
        )}
        <ActionMenu
          label="Power"
          align="left"
          actions={[
            {
              id: 'enter-dfu',
              label: busy === 'dfu' ? 'Entering recovery' : 'Enter recovery',
              detail: 'Prepare the keyboard for update',
              tone: 'warn',
              disabled: busy !== null || flashBusy,
              onSelect: () => void enterDfu(),
            },
            ...(native
              ? [
                  {
                    id: 'return-to-keyboard',
                    label: 'Return to keyboard',
                    detail: 'Leave recovery mode',
                    disabled: busy !== null || flashBusy,
                    onSelect: onRebootToKeyboard,
                  },
                ]
              : []),
            {
              id: 'reboot-firmware',
              label: busy === 'reboot' ? 'Restarting' : 'Restart',
              detail: 'Restart the keyboard',
              disabled: busy !== null || flashBusy,
              onSelect: () => void reboot(),
            },
          ]}
        />
        <InfoHint label="Power action details">
          Recovery and restart actions can briefly disconnect the keyboard.
        </InfoHint>
      </div>

      {!native && (
        <div className="mt-3 inline-flex items-center gap-2 border border-[#1b222a] bg-[#020304] px-3 py-2 text-xs text-slate-400">
          Desktop app required for board updates
          <InfoHint label="Desktop update details">
            Open keeberry in the desktop app to install bundled updates.
          </InfoHint>
        </div>
      )}
    </Panel>
  );
}
