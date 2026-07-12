// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useRef, useState } from 'react';
import type { DeviceInfo, KcpClient } from '../kcp';
import {
  flashFirmware,
  isNativeBuild,
  loadBundledFirmware,
  onFlashProgress,
  rebootToNormal,
  type BundledFirmware,
  type FlashProgress,
} from './nativeFlash';
import { backupBeforeFlash } from './preFlashBackup';
import { readPreserveAcrossFlash } from './preserveAcrossFlash';

/** How long to let the board re-enumerate after a flash before reconnecting. */
const RECONNECT_DELAY_MS = 1800;

export interface UseFirmwareFlash {
  /** True in the desktop (Tauri) build, where flashing is possible at all. */
  native: boolean;
  /** The firmware version the app bundles, or null when unknown. */
  bundled: BundledFirmware | null;
  /** The current flash round-trip step, or null when idle. */
  progress: FlashProgress | null;
  /** True while a flash/reboot is in flight (disables the trigger buttons). */
  busy: boolean;
  /**
   * One-click update from a connected device: back up settings (when the preserve
   * toggle is on), enter DFU (kcp 0xF0), then flash. `deviceInfo` describes the
   * connected device for the pre-flash backup.
   */
  updateFirmware: (client: KcpClient, deviceInfo: DeviceInfo) => Promise<void>;
  /** Flash a board that is already in the bootloader (skips the enter-DFU step). */
  flashInBootloader: () => Promise<void>;
  /** Reboot a board out of the bootloader back into its firmware. */
  rebootToKeyboard: () => Promise<void>;
  /** Clear the progress banner (after a terminal done/error step). */
  dismissProgress: () => void;
}

function errorMessage(err: unknown): string {
  return err instanceof Error ? err.message : String(err);
}

/**
 * Owns the firmware-flash round-trip at a level that survives the keyboard
 * disconnecting. The one-click update enters DFU (kcp 0xF0), which drops the
 * device off USB and unmounts the per-connection panels — so the flow, its
 * progress (driven by `flash-progress` events) and the post-flash reconnect must
 * live here in App, not in a panel the disconnect tears down. The flasher itself
 * is a separate sidecar process, unaffected by the kcp connection going away.
 *
 * `reconnect` is the app's connect action, called once after a successful flash
 * so the freshly-rebooted keyboard comes back without the user clicking Connect.
 */
export function useFirmwareFlash(reconnect: () => void): UseFirmwareFlash {
  const native = isNativeBuild();
  const [bundled, setBundled] = useState<BundledFirmware | null>(null);
  const [progress, setProgress] = useState<FlashProgress | null>(null);
  const [busy, setBusy] = useState(false);
  const reconnectTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  // Load the bundled manifest once; null in the browser or an unstaged build.
  useEffect(() => {
    let active = true;
    void loadBundledFirmware().then((manifest) => {
      if (active) {
        setBundled(manifest);
      }
    });
    return () => {
      active = false;
    };
  }, []);

  // Mirror every flash-progress event into the banner for the whole app session,
  // independent of which panels are mounted at the time.
  useEffect(() => {
    if (!native) {
      return;
    }
    let active = true;
    let unlisten: (() => void) | undefined;
    void onFlashProgress((p) => setProgress(p)).then((fn) => {
      if (active) {
        unlisten = fn;
      } else {
        fn();
      }
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [native]);

  // Drop any pending reconnect on unmount.
  useEffect(() => {
    return () => {
      if (reconnectTimer.current) {
        clearTimeout(reconnectTimer.current);
      }
    };
  }, []);

  const scheduleReconnect = useCallback(() => {
    if (reconnectTimer.current) {
      clearTimeout(reconnectTimer.current);
    }
    reconnectTimer.current = setTimeout(() => {
      reconnectTimer.current = null;
      setProgress(null);
      reconnect();
    }, RECONNECT_DELAY_MS);
  }, [reconnect]);

  // Run a flash action, mapping a rejection to a terminal error step. The success
  // path schedules the reconnect; the Rust `done`/`error` events have already set
  // the matching banner text along the way.
  const runFlash = useCallback(
    async (action: () => Promise<void>) => {
      setBusy(true);
      try {
        await action();
        scheduleReconnect();
      } catch (err) {
        setProgress({ phase: 'error', message: errorMessage(err) });
      } finally {
        setBusy(false);
      }
    },
    [scheduleReconnect],
  );

  const updateFirmware = useCallback(
    async (client: KcpClient, deviceInfo: DeviceInfo) => {
      await runFlash(async () => {
        // Preserve keymap & settings across the flash exactly as the manual DFU
        // path (SystemPanel) does: stash the live config before the device drops
        // off USB. A failed backup throws, which runFlash turns into a terminal
        // error and skips the reconnect — so settings are never reset to defaults
        // while the preserve toggle reads "On". Done inside runFlash so the busy
        // flag covers the (multi-round-trip) backup, like the manual path's.
        const preserved = await backupBeforeFlash(client, deviceInfo, readPreserveAcrossFlash());
        if (!preserved.ok) {
          throw new Error(preserved.message);
        }
        // Optimistic step before the device drops off USB; the Rust flasher
        // re-emits `entering`/`waiting` once it takes over.
        setProgress({ phase: 'entering', message: 'Entering the bootloader…' });
        await client.enterDfu();
        await flashFirmware();
      });
    },
    [runFlash],
  );

  const flashInBootloader = useCallback(async () => {
    setProgress({ phase: 'waiting', message: 'Looking for the bootloader…' });
    await runFlash(() => flashFirmware());
  }, [runFlash]);

  const rebootToKeyboard = useCallback(async () => {
    setProgress({ phase: 'rebooting', message: 'Rebooting into the keyboard…' });
    await runFlash(() => rebootToNormal());
  }, [runFlash]);

  const dismissProgress = useCallback(() => {
    if (reconnectTimer.current) {
      clearTimeout(reconnectTimer.current);
      reconnectTimer.current = null;
    }
    setProgress(null);
  }, []);

  return {
    native,
    bundled,
    progress,
    busy,
    updateFirmware,
    flashInBootloader,
    rebootToKeyboard,
    dismissProgress,
  };
}
