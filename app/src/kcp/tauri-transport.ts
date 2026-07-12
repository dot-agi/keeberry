// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Native (Tauri) implementation of the {@link Transport} seam — the sibling of
 * {@link WebHidTransport}. The macOS webview (WKWebView) has no WebHID, so the
 * same React app reaches the keyboard through the Rust HID bridge instead: it
 * `invoke`s the `hid_*` commands and `listen`s for the `kcp-report` /
 * `kcp-disconnect` events (see `src-tauri/src/hid.rs`). Nothing above this layer
 * knows whether it is talking to WebHID or Rust.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  DisconnectListener,
  ReportListener,
  Transport,
  TransportDevice,
  Unsubscribe,
} from './transport-iface';

/** One enumerated kcp device from `hid_list`, shaped to drive the native picker. */
export interface NativeHidDevice {
  /** Opaque OS path, passed back to `hid_open` to reopen this exact interface. */
  readonly path: string;
  /** Human-friendly name for the app-rendered chooser. */
  readonly name: string;
}

/** Inbound 32-byte report event emitted by the Rust worker. */
const EVENT_REPORT = 'kcp-report';
/** Device-left-the-bus event (unplug / MCU reset) emitted by the Rust worker. */
const EVENT_DISCONNECT = 'kcp-disconnect';
/**
 * How often {@link TauriHidTransport.watchForDevice} re-enumerates while waiting for
 * a board to return. The bridge has no device-arrival event (only the open device's
 * `kcp-disconnect`), so arrival is found by polling `hid_list`; 1 s is responsive
 * for a re-plug yet negligible cost.
 */
const RECONNECT_POLL_MS = 1000;

/**
 * A {@link TransportDevice} backed by the Rust bridge. It owns the Tauri event
 * subscriptions and fans each event out to the in-memory listeners the kcp
 * connection registers.
 *
 * The Tauri `listen` API is asynchronous, but the {@link TransportDevice} contract
 * (like WebHID's `addEventListener`) is synchronous. To bridge that without a
 * startup race, the device registers its event listeners eagerly in {@link open}
 * (awaited before any write) and dispatches to in-memory subscriber sets that
 * {@link subscribe} / {@link onDisconnect} populate synchronously. Because the
 * firmware only ever sends a report in reply to a request — and the first request
 * follows `open()` and `subscribe()` — no inbound report can outrun its listener.
 */
class TauriHidTransportDevice implements TransportDevice {
  private readonly reportListeners = new Set<ReportListener>();
  private readonly disconnectListeners = new Set<DisconnectListener>();
  private unlistenReport?: UnlistenFn;
  private unlistenDisconnect?: UnlistenFn;
  private opened = false;

  constructor(private readonly info: NativeHidDevice) {}

  get name(): string {
    return this.info.name;
  }

  async open(): Promise<void> {
    if (this.opened) {
      return;
    }
    // Register the inbound listeners before the device can produce any report.
    this.unlistenReport = await listen<number[]>(EVENT_REPORT, (event) => {
      const bytes = Uint8Array.from(event.payload);
      for (const listener of this.reportListeners) {
        listener(bytes);
      }
    });
    this.unlistenDisconnect = await listen(EVENT_DISCONNECT, () => {
      // The device is gone: notify, then drop the now-useless subscriptions.
      const listeners = [...this.disconnectListeners];
      this.teardownEvents();
      for (const listener of listeners) {
        listener();
      }
    });
    await invoke('hid_open', { path: this.info.path });
    this.opened = true;
  }

  /** No report ID on this interface; the Rust side prepends report-id 0 on write. */
  async write(report: Uint8Array<ArrayBuffer>): Promise<void> {
    await invoke('hid_write', { bytes: Array.from(report) });
  }

  subscribe(listener: ReportListener): Unsubscribe {
    this.reportListeners.add(listener);
    return () => {
      this.reportListeners.delete(listener);
    };
  }

  onDisconnect(listener: DisconnectListener): Unsubscribe {
    this.disconnectListeners.add(listener);
    return () => {
      this.disconnectListeners.delete(listener);
    };
  }

  async close(): Promise<void> {
    if (this.opened) {
      await invoke('hid_close');
    }
    this.teardownEvents();
  }

  /** Detach the Tauri event subscriptions and mark the device closed. */
  private teardownEvents(): void {
    this.opened = false;
    this.unlistenReport?.();
    this.unlistenReport = undefined;
    this.unlistenDisconnect?.();
    this.unlistenDisconnect = undefined;
  }
}

/**
 * The native transport: enumerates keeberry devices over the Rust bridge. With no
 * OS chooser to defer to, {@link requestDevice} auto-picks when exactly one device
 * is present; selecting among several is the app's job, driven by
 * {@link listDevices} + {@link deviceFor} (see `useKcpDevice`).
 */
export class TauriHidTransport implements Transport {
  /** True only inside the Tauri runtime (its IPC globals are injected on `window`). */
  isSupported(): boolean {
    return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
  }

  /** Enumerate the keeberry kcp interface (VID/PID + usage page) via Rust. */
  listDevices(): Promise<NativeHidDevice[]> {
    return invoke<NativeHidDevice[]>('hid_list');
  }

  /** Wrap an enumerated device so it can be opened and driven. */
  deviceFor(info: NativeHidDevice): TransportDevice {
    return new TauriHidTransportDevice(info);
  }

  async requestDevice(): Promise<TransportDevice | null> {
    const devices = await this.listDevices();
    return devices.length === 1 ? this.deviceFor(devices[0]) : null;
  }

  /**
   * Find a currently-enumerated keeberry kcp device without prompting, the native
   * mirror of {@link WebHidTransport.getKnownDevice}: the post-switch auto-reconnect
   * reopens the board the moment Rust lists it again. Returns the first device
   * (there is normally exactly one after a re-enumeration), or `null` when none is
   * present yet.
   */
  async getKnownDevice(): Promise<TransportDevice | null> {
    const devices = await this.listDevices();
    return devices.length > 0 ? this.deviceFor(devices[0]) : null;
  }

  /**
   * Signal `onArrival` on a fixed interval so a watcher can re-check for the
   * keeberry returning; the caller re-enumerates via {@link getKnownDevice} on each
   * tick. Polling stands in for the device-arrival event the bridge does not emit.
   * Returns an unsubscribe that stops the timer.
   */
  watchForDevice(onArrival: () => void): Unsubscribe {
    const timer = setInterval(onArrival, RECONNECT_POLL_MS);
    return () => clearInterval(timer);
  }
}
