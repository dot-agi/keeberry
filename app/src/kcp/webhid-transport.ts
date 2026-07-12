// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * WebHID implementation of the {@link Transport} seam. This is the only module in
 * the kcp client that touches `navigator.hid`: it prompts the user through the
 * browser chooser, opens the picked device, and adapts WebHID's report I/O and
 * `disconnect` event to the transport interface. The native (Tauri) build supplies
 * a sibling implementation against the same interface, so nothing above this layer
 * knows which one it is talking to.
 */
import { USAGE, USAGE_PAGE } from './protocol';
import type {
  DisconnectListener,
  ReportListener,
  Transport,
  TransportDevice,
  Unsubscribe,
} from './transport-iface';

/**
 * The USB identities that carry the kcp interface. Two distinct devices expose the
 * very same `0xFF60`/`0x61` usage:
 *
 * - the keeberry keyboard over USB (the pid.codes open-source TEST allocation,
 *   mirrored in the firmware's `usb.rs` and the native bridge's `hid.rs`); and
 * - the stock Akko 2.4 GHz dongle, which bridges kcp over the radio byte-for-byte.
 *
 * The dongle keeps its own Akko USB identity while bridging a keeberry keyboard — a
 * 2.4 GHz pair does NOT re-brand it to keeberry's VID/PID. Verified on hardware: a
 * keeberry keyboard answers kcp over the dongle (GET_DEVICE_INFO reports transport =
 * 2.4 GHz) while the dongle still enumerates as `0x342D`/`0xE4D7` "Akko 2.4G VIA KB".
 * So the wireless link is reachable only by also recognizing the dongle's identity;
 * without it the chooser is empty once the keyboard's own USB is unplugged.
 */
const KEEBERRY_VENDOR_ID = 0x1209;
const KEEBERRY_PRODUCT_ID = 0x0001;
const AKKO_DONGLE_VENDOR_ID = 0x342d;
const AKKO_DONGLE_PRODUCT_ID = 0xe4d7;

/** The (vendorId, productId) pairs that expose the kcp interface (keyboard + dongle). */
const KCP_DEVICE_IDS: readonly { vendorId: number; productId: number }[] = [
  { vendorId: KEEBERRY_VENDOR_ID, productId: KEEBERRY_PRODUCT_ID },
  { vendorId: AKKO_DONGLE_VENDOR_ID, productId: AKKO_DONGLE_PRODUCT_ID },
];

/**
 * WebHID device filters: each kcp identity in {@link KCP_DEVICE_IDS} pinned to the
 * kcp usage page and usage. Matching the usage page alone would surface every QMK
 * `0xFF60` board in the chooser — and picking a non-kcp one makes every `transact`
 * time out — so the set is an explicit allowlist of the keyboard and its dongle
 * rather than a usage-only match. The native enumerate filter
 * (`src-tauri/src/hid.rs`) needs the same dongle identity to reach 2.4 GHz.
 */
const KCP_FILTERS: HIDDeviceFilter[] = KCP_DEVICE_IDS.map(({ vendorId, productId }) => ({
  vendorId,
  productId,
  usagePage: USAGE_PAGE,
  usage: USAGE,
}));

/**
 * Whether an already-permitted device exposes the kcp interface — the same match as
 * {@link KCP_FILTERS} (a known kcp identity in {@link KCP_DEVICE_IDS} plus the
 * `0xFF60`/`0x61` collection), but applied to a device the page already has access
 * to (so its `collections` are known). Used to pick the keyboard or its dongle out
 * of {@link Navigator.hid}'s granted set without re-prompting.
 */
function isKcpDevice(device: HIDDevice): boolean {
  return (
    KCP_DEVICE_IDS.some(
      (id) => id.vendorId === device.vendorId && id.productId === device.productId,
    ) && device.collections.some((c) => c.usagePage === USAGE_PAGE && c.usage === USAGE)
  );
}

/**
 * A {@link TransportDevice} backed by a WebHID `HIDDevice`. It owns the
 * `inputreport` and `disconnect` listeners and translates them to transport
 * callbacks: an inbound report becomes its report bytes (report id 0, the 32-byte
 * IN frame), and the bus-wide `disconnect` event is filtered down to this device.
 */
class WebHidTransportDevice implements TransportDevice {
  constructor(private readonly device: HIDDevice) {}

  get name(): string {
    return this.device.productName || 'keeberry device';
  }

  async open(): Promise<void> {
    if (!this.device.opened) {
      await this.device.open();
    }
  }

  /** No report ID on this interface, so the report id is 0. */
  write(report: Uint8Array<ArrayBuffer>): Promise<void> {
    return this.device.sendReport(0, report);
  }

  subscribe(listener: ReportListener): Unsubscribe {
    const handler = (event: HIDInputReportEvent): void => {
      const { data } = event;
      listener(new Uint8Array(data.buffer, data.byteOffset, data.byteLength));
    };
    this.device.addEventListener('inputreport', handler);
    return () => {
      this.device.removeEventListener('inputreport', handler);
    };
  }

  onDisconnect(listener: DisconnectListener): Unsubscribe {
    const handler = (event: HIDConnectionEvent): void => {
      if (event.device === this.device) {
        listener();
      }
    };
    navigator.hid.addEventListener('disconnect', handler);
    return () => {
      navigator.hid.removeEventListener('disconnect', handler);
    };
  }

  async close(): Promise<void> {
    if (this.device.opened) {
      await this.device.close();
    }
  }
}

/** The browser transport: selects keeberry devices through the WebHID chooser. */
export class WebHidTransport implements Transport {
  /** True when this browser exposes the WebHID API. */
  isSupported(): boolean {
    return typeof navigator !== 'undefined' && 'hid' in navigator;
  }

  async requestDevice(): Promise<TransportDevice | null> {
    if (!this.isSupported()) {
      throw new Error('WebHID is not supported in this browser');
    }
    const devices = await navigator.hid.requestDevice({ filters: KCP_FILTERS });
    const device = devices[0];
    return device ? new WebHidTransportDevice(device) : null;
  }

  /**
   * Find an already-permitted kcp device (keyboard or dongle) without prompting.
   * Once the user has granted access through {@link requestDevice}, the browser
   * keeps the permission, so a board that re-enumerates back to Normal (after
   * leaving MIDI / XInput) can be reopened with no chooser and no user gesture — the
   * path the post-switch auto-reconnect takes. Resolves to the kcp interface wrapped
   * as a {@link TransportDevice}, or `null` when no granted kcp device is present.
   */
  async getKnownDevice(): Promise<TransportDevice | null> {
    if (!this.isSupported()) {
      return null;
    }
    const match = (await navigator.hid.getDevices()).find(isKcpDevice);
    return match ? new WebHidTransportDevice(match) : null;
  }

  /**
   * Call `onArrival` whenever a device the page can access joins the bus, so a
   * watcher can re-check for the keeberry returning. The browser fires `connect`
   * only for already-permitted devices, so this never wakes on an unrelated board;
   * the caller confirms identity via {@link getKnownDevice}. Returns an unsubscribe
   * that detaches the listener.
   */
  watchForDevice(onArrival: () => void): Unsubscribe {
    if (!this.isSupported()) {
      return () => {};
    }
    const handler = (): void => onArrival();
    navigator.hid.addEventListener('connect', handler);
    return () => {
      navigator.hid.removeEventListener('connect', handler);
    };
  }
}
