// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Transport seam for the kcp client. {@link KcpConnection} and {@link KcpClient}
 * operate over these interfaces rather than touching `navigator.hid` directly, so
 * the same React app can drive a keeberry keyboard through WebHID in the browser
 * (see {@link WebHidTransport}) and, in the native (Tauri) build, through Rust —
 * whose macOS webview has no WebHID. The interface captures exactly what the
 * client needs: select a device, then on that device open it, write a 32-byte OUT
 * report, receive inbound reports, observe disconnect, and close.
 */

/** Receives one inbound report's bytes (a 32-byte IN frame). */
export type ReportListener = (data: Uint8Array) => void;

/** Fires once when the device leaves the bus (unplug or MCU reset). */
export type DisconnectListener = () => void;

/** Detaches a listener registered with `subscribe` or `onDisconnect`. */
export type Unsubscribe = () => void;

/**
 * A selected keeberry device. {@link open} readies it for I/O, {@link write}
 * sends one 32-byte OUT report, {@link subscribe} delivers inbound reports,
 * {@link onDisconnect} signals when the device drops off the bus, and
 * {@link close} releases it.
 */
export interface TransportDevice {
  /** A human-friendly device name for the UI. */
  readonly name: string;
  /** Ready the device for I/O; a no-op if it is already open. */
  open(): Promise<void>;
  /**
   * Send one 32-byte OUT report. The frame is an owned `ArrayBuffer`-backed view
   * (as {@link encodeRequest} produces), which WebHID's `sendReport` requires.
   */
  write(report: Uint8Array<ArrayBuffer>): Promise<void>;
  /** Subscribe to inbound reports; returns a function that unsubscribes. */
  subscribe(listener: ReportListener): Unsubscribe;
  /** Observe disconnect (unplug / reset); returns a function that unsubscribes. */
  onDisconnect(listener: DisconnectListener): Unsubscribe;
  /** Release the device; a no-op if it is already closed. */
  close(): Promise<void>;
}

/**
 * Enumerates and selects keeberry devices. The browser implementation prompts the
 * user through the WebHID chooser; the native build enumerates over Rust and
 * renders its own picker against this same interface.
 */
export interface Transport {
  /** Whether this transport is available in the current runtime. */
  isSupported(): boolean;
  /**
   * Prompt the user to pick a keeberry device. Resolves to `null` if the user
   * dismisses the chooser without selecting one.
   */
  requestDevice(): Promise<TransportDevice | null>;
}
