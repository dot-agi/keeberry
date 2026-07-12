// SPDX-License-Identifier: GPL-2.0-or-later
//! keeberry native desktop shell.
//!
//! The same React app that runs in the browser is loaded here in a Tauri (WKWebView)
//! window. WKWebView has no WebHID, so the webview reaches the keyboard through the
//! Rust HID bridge in [`hid`]: four commands (`hid_list` / `hid_open` / `hid_write`
//! / `hid_close`) plus a `kcp-report` event carrying each inbound 32-byte report.
//!
//! Flashing is native-only too (the DFU bootloader is USB-DFU, not HID): the
//! [`flash`] commands shell out to the bundled wb32-dfu-updater_cli sidecar via
//! the shell plugin, streaming `flash-progress` events for the DFU round-trip.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod flash;
mod hid;

fn main() {
    tauri::Builder::default()
        // The shell plugin runs the flasher sidecar; the flash commands call it
        // from Rust, so no shell execute permission is exposed to the webview.
        .plugin(tauri_plugin_shell::init())
        .manage(hid::HidState::default())
        .invoke_handler(tauri::generate_handler![
            hid::hid_list,
            hid::hid_open,
            hid::hid_write,
            hid::hid_close,
            flash::reboot_to_normal,
            flash::flash_firmware,
            flash::bundled_firmware,
        ])
        .run(tauri::generate_context!())
        .expect("error while running the keeberry desktop app");
}
