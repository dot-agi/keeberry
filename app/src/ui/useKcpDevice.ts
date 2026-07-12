// SPDX-License-Identifier: GPL-2.0-or-later
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  KcpClient,
  TauriHidTransport,
  UsbMode,
  WebHidTransport,
  planFlashRestore,
  writeFullConfig,
  type Capabilities,
  type DeviceInfo,
  type NativeHidDevice,
  type ProtocolVersion,
} from '../kcp';
import { readPreserveAcrossFlash } from './preserveAcrossFlash';
import { clearPendingRestore, readPendingRestore } from './pendingRestore';
import { restoreUnicodeMap } from './unicodeMap';

/**
 * The transport this build connects through, chosen once at module load. Inside
 * the Tauri runtime the webview has no WebHID, so it reaches the keyboard over the
 * Rust bridge ({@link TauriHidTransport}); in a browser it uses {@link WebHidTransport}.
 * Both satisfy the same {@link Transport} seam, so everything below is identical
 * apart from the native multi-device picker.
 */
const transport: WebHidTransport | TauriHidTransport =
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window
    ? new TauriHidTransport()
    : new WebHidTransport();

/** Everything the UI shows once a device is connected and queried. */
export interface DeviceSnapshot {
  name: string;
  protocolVersion: ProtocolVersion;
  capabilities: Capabilities;
  deviceInfo: DeviceInfo;
}

/** A post-(re)connect persist-across-flash notification (the restore outcome). */
export interface RestoreToast {
  tone: 'ok' | 'warn';
  message: string;
}

export type ConnectionState =
  | 'unsupported'
  | 'idle'
  | 'connecting'
  | 'selecting'
  | 'connected'
  // The link dropped because we switched the USB personality (MIDI / XInput, or
  // back to Normal): an expected re-enumeration, not a fault. The mode-specific
  // waiting panel is shown while the watcher waits for the kcp interface to return.
  | 'switched'
  | 'error';

export interface UseKcpDevice {
  state: ConnectionState;
  snapshot: DeviceSnapshot | null;
  /** The live client while connected, for the group editors to issue requests. */
  client: KcpClient | null;
  error: string | null;
  /** Clear the connection error (e.g. when dismissing the flash banner that stood in for it). */
  clearError: () => void;
  /** The persist-across-flash restore outcome, shown until dismissed. */
  restoreToast: RestoreToast | null;
  dismissRestoreToast: () => void;
  connect: () => Promise<void>;
  disconnect: () => Promise<void>;
  /**
   * In the native build, the keeberry devices to choose from when more than one is
   * attached (state `'selecting'`); `null` otherwise, and always in the browser,
   * whose own WebHID chooser handles selection.
   */
  devices: NativeHidDevice[] | null;
  /** Connect to a specific native device picked from {@link devices}. */
  selectDevice: (device: NativeHidDevice) => Promise<void>;
  /**
   * While in state `'switched'`, the personality the keyboard re-enumerated into
   * (MIDI / XInput, or Normal while it returns); `null` in every other state. Drives
   * the mode-specific waiting panel.
   */
  switchedMode: UsbMode | null;
  /**
   * Re-open the keeberry kcp interface without a chooser once it is back on the bus
   * (after leaving MIDI / XInput, or once Normal re-enumerates). Safe to call
   * repeatedly: a no-op while a connect is already in flight or a device is
   * connected. Wired to both the arrival watcher and the switched panel's button.
   */
  reconnect: () => Promise<void>;
  /**
   * Arm the next disconnect to be read as the acknowledgement of a USB-mode switch,
   * so it transitions to the mode-specific `'switched'` state rather than surfacing a
   * connection error. The USB mode panel calls this immediately before issuing
   * `SET_USB_MODE`.
   */
  beginUsbModeSwitch: (mode: UsbMode) => void;
}

/**
 * Persist-across-flash restore, run once a device is connected and described. If
 * the toggle is on and a `pendingRestore` backup is waiting, compare its schema to
 * the now-connected firmware: a match writes the full config and saves it (then
 * clears the backup); a mismatch keeps the backup (so it can still be exported)
 * and reports that the new firmware reset settings to defaults. Returns the toast
 * to show, or `null` when there is nothing to restore.
 */
async function attemptFlashRestore(
  client: KcpClient,
  deviceInfo: DeviceInfo,
): Promise<RestoreToast | null> {
  if (!readPreserveAcrossFlash()) {
    return null;
  }
  const backup = readPendingRestore();
  if (!backup) {
    return null;
  }

  const plan = planFlashRestore(backup, deviceInfo);
  if (plan.action === 'skip') {
    // Schema changed: do NOT restore, but keep the backup so the user can export it.
    return { tone: 'warn', message: plan.message };
  }

  try {
    await writeFullConfig(client, backup.config);
    await client.configSave();
    clearPendingRestore();
    return { tone: 'ok', message: 'Settings restored after flash.' };
  } catch (err) {
    // Keep the backup so the restore can be retried or exported.
    const reason = err instanceof Error ? err.message : String(err);
    return {
      tone: 'warn',
      message: `Could not restore settings after flash: ${reason} Your backup is kept.`,
    };
  }
}

/**
 * Owns the device connection lifecycle (WebHID in the browser, the Rust bridge in
 * the native build), the INFO snapshot, the native device picker and the persist-
 * across-flash restore. The component tree stays declarative: it reads
 * `state`/`snapshot`/`error`/`restoreToast`/`devices` and calls
 * `connect`/`selectDevice`/`disconnect`.
 */
export function useKcpDevice(): UseKcpDevice {
  const [state, setState] = useState<ConnectionState>(() =>
    transport.isSupported() ? 'idle' : 'unsupported',
  );
  const [snapshot, setSnapshot] = useState<DeviceSnapshot | null>(null);
  const [client, setClient] = useState<KcpClient | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [restoreToast, setRestoreToast] = useState<RestoreToast | null>(null);
  const [devices, setDevices] = useState<NativeHidDevice[] | null>(null);
  const [switchedMode, setSwitchedMode] = useState<UsbMode | null>(null);
  const clientRef = useRef<KcpClient | null>(null);
  /**
   * The USB personality we last asked the board to switch to, read by the next
   * disconnect to tell an expected mode-switch re-enumeration apart from an unplug.
   * A ref (not state) because {@link onDisconnect} — wired into the client at connect
   * time — must read the value current at the drop, not one captured in a closure.
   */
  const pendingSwitchRef = useRef<UsbMode | null>(null);
  /** Guards {@link reconnect} so overlapping arrival signals open only one device. */
  const reconnectingRef = useRef(false);

  /** Tear down the live-connection state shared by every disconnect/reset path. */
  const clearDeviceState = useCallback(() => {
    clientRef.current = null;
    setClient(null);
    setSnapshot(null);
    setRestoreToast(null);
    setDevices(null);
  }, []);

  const resetToIdle = useCallback(
    (message: string | null) => {
      clearDeviceState();
      setError(message);
      setSwitchedMode(null);
      pendingSwitchRef.current = null;
      setState(transport.isSupported() ? 'idle' : 'unsupported');
    },
    [clearDeviceState],
  );

  const onDisconnect = useCallback(() => {
    const pending = pendingSwitchRef.current;
    pendingSwitchRef.current = null;
    if (pending === null) {
      resetToIdle('Device disconnected.');
      return;
    }
    // The drop is the acknowledgement of the USB-mode switch we just issued, not a
    // fault: re-enumerating into MIDI / XInput (or back to Normal) tears down this
    // link by design. Show the mode-specific waiting state and let the watcher
    // reopen the kcp interface when it returns, rather than flagging an error.
    clearDeviceState();
    setError(null);
    setSwitchedMode(pending);
    setState('switched');
  }, [clearDeviceState, resetToIdle]);

  /**
   * The shared connection tail: describe the device, run the persist-across-flash
   * restore before the editors mount (so they read the restored state rather than
   * racing the writes), and publish the snapshot. Throws on failure for the caller.
   */
  const finishConnect = useCallback(async (connected: KcpClient) => {
    const [protocolVersion, capabilities, deviceInfo] = await Promise.all([
      connected.getProtocolVersion(),
      connected.getCapabilities(),
      connected.getDeviceInfo(),
    ]);
    const toast = await attemptFlashRestore(connected, deviceInfo);
    if (capabilities.groups.unicode) {
      // The Unicode codepoint map lives in device RAM (cleared on power-cycle), so re-upload
      // the locally cached map on every (re)connect — independent of whether the Unicode
      // panel is ever opened, otherwise the only thing that restores it. Best-effort: a
      // failure is retried when the panel mounts and must not fail the connection.
      await restoreUnicodeMap(connected).catch(() => {});
    }
    clientRef.current = connected;
    setClient(connected);
    setSnapshot({ name: connected.name, protocolVersion, capabilities, deviceInfo });
    setRestoreToast(toast);
    setSwitchedMode(null);
    setState('connected');
  }, []);

  const failConnect = useCallback((err: unknown) => {
    clientRef.current = null;
    setClient(null);
    setSnapshot(null);
    setError(err instanceof Error ? err.message : String(err));
    setRestoreToast(null);
    setState('error');
  }, []);

  const connect = useCallback(async () => {
    if (!transport.isSupported()) {
      setState('unsupported');
      return;
    }
    setError(null);
    setRestoreToast(null);
    setDevices(null);
    setSwitchedMode(null);
    pendingSwitchRef.current = null;
    setState('connecting');
    try {
      if (transport instanceof TauriHidTransport) {
        const found = await transport.listDevices();
        if (found.length === 0) {
          setError('No keeberry device found. Connect your keyboard and try again.');
          setState('error');
          return;
        }
        if (found.length > 1) {
          // Several keeberry devices are attached and there is no native chooser,
          // so hand the list to the app to render its own picker.
          setDevices(found);
          setState('selecting');
          return;
        }
        const device = transport.deviceFor(found[0]);
        await finishConnect(await KcpClient.fromDevice(device, { onDisconnect }));
        return;
      }
      const connected = await KcpClient.request(transport, { onDisconnect });
      if (!connected) {
        // The user dismissed the WebHID chooser without picking a device.
        setState('idle');
        return;
      }
      await finishConnect(connected);
    } catch (err) {
      failConnect(err);
    }
  }, [finishConnect, failConnect, onDisconnect]);

  const selectDevice = useCallback(
    async (choice: NativeHidDevice) => {
      if (!(transport instanceof TauriHidTransport)) {
        return;
      }
      setError(null);
      setRestoreToast(null);
      setDevices(null);
      setState('connecting');
      try {
        const device = transport.deviceFor(choice);
        await finishConnect(await KcpClient.fromDevice(device, { onDisconnect }));
      } catch (err) {
        failConnect(err);
      }
    },
    [finishConnect, failConnect, onDisconnect],
  );

  const disconnect = useCallback(async () => {
    const client = clientRef.current;
    resetToIdle(null);
    if (client) {
      await client.close();
    }
  }, [resetToIdle]);

  const clearError = useCallback(() => setError(null), []);

  const dismissRestoreToast = useCallback(() => setRestoreToast(null), []);

  const beginUsbModeSwitch = useCallback((mode: UsbMode) => {
    pendingSwitchRef.current = mode;
  }, []);

  const reconnect = useCallback(async () => {
    // Skip if a device is already connected or an attempt is in flight, so the
    // many arrival signals (the immediate try, the WebHID connect event, every
    // native poll tick) coalesce into a single open.
    if (reconnectingRef.current || clientRef.current) {
      return;
    }
    reconnectingRef.current = true;
    try {
      const device = await transport.getKnownDevice();
      if (device && !clientRef.current) {
        await finishConnect(await KcpClient.fromDevice(device, { onDisconnect }));
      }
    } catch {
      // The keeberry was seen but could not be opened yet (still settling after the
      // re-enumeration, or a transient race): stay in 'switched' and let the next
      // arrival signal retry. The manual button is the user's fallback.
    } finally {
      reconnectingRef.current = false;
    }
  }, [finishConnect, onDisconnect]);

  // While waiting out a USB-mode switch, watch for the kcp interface returning and
  // reopen it with no chooser. One immediate attempt covers a device that is already
  // back (e.g. MIDI, which keeps kcp live); the transport's arrival signal — the
  // WebHID `connect` event or the native enumeration poll — drives the later return
  // from XInput once the user runs the firmware escape combo or replugs.
  useEffect(() => {
    if (state !== 'switched') {
      return;
    }
    void reconnect();
    return transport.watchForDevice(() => void reconnect());
  }, [state, reconnect]);

  useEffect(() => {
    return () => {
      void clientRef.current?.close();
    };
  }, []);

  return {
    state,
    snapshot,
    client,
    error,
    clearError,
    restoreToast,
    dismissRestoreToast,
    connect,
    disconnect,
    devices,
    selectDevice,
    switchedMode,
    reconnect,
    beginUsbModeSwitch,
  };
}
