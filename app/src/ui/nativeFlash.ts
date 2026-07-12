// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * Native firmware-flashing bridge — the webview side of `src-tauri/src/flash.rs`.
 *
 * Flashing is native-only: it drives the bundled wb32-dfu-updater_cli sidecar
 * through Tauri commands and listens for `flash-progress` events. WebHID (the
 * browser) cannot reach the DFU bootloader, so in a browser these calls are never
 * made — the UI hides the native actions and points the user at the desktop app.
 */
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

/** Steps of the flash round-trip, mirroring the Rust `FlashProgress.phase`. */
export type FlashPhase = 'entering' | 'waiting' | 'flashing' | 'rebooting' | 'done' | 'error';

/** One `flash-progress` event payload. */
export interface FlashProgress {
  phase: FlashPhase;
  message: string;
}

/** The firmware image the app bundles, from `resources/firmware.json`. */
export interface BundledFirmware {
  version: string;
}

/** The `flash-progress` event name (matches `EVENT_PROGRESS` in flash.rs). */
const EVENT_FLASH_PROGRESS = 'flash-progress';

/** True only inside the Tauri runtime (its IPC globals are injected on `window`). */
export function isNativeBuild(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

/**
 * Read the bundled firmware manifest. Returns `null` when unavailable — the
 * browser build (no Tauri) or a native build that was not staged with an image —
 * so callers treat "what we ship" as simply unknown rather than erroring.
 */
export async function loadBundledFirmware(): Promise<BundledFirmware | null> {
  if (!isNativeBuild()) {
    return null;
  }
  try {
    return await invoke<BundledFirmware>('bundled_firmware');
  } catch {
    return null;
  }
}

/** Subscribe to flash-progress events; resolves with an unlisten function. */
export function onFlashProgress(handler: (progress: FlashProgress) => void): Promise<UnlistenFn> {
  return listen<FlashProgress>(EVENT_FLASH_PROGRESS, (event) => handler(event.payload));
}

/** Reboot the board out of DFU into its firmware (`wb32-dfu-updater_cli -R`). */
export function rebootToNormal(): Promise<void> {
  return invoke('reboot_to_normal');
}

/**
 * Wait for the DFU device, write the bundled image, then reset. Resolves once the
 * board is rebooting into the new firmware; rejects (and emits an `error` event)
 * if the bootloader never appears or the flasher fails. Progress is reported via
 * {@link onFlashProgress}.
 */
export function flashFirmware(): Promise<void> {
  return invoke('flash_firmware');
}
