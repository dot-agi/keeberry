// SPDX-License-Identifier: GPL-2.0-or-later
//! Native firmware flashing over the bundled wb32-dfu-updater_cli sidecar.
//!
//! WebHID (browser) and the native HID bridge both speak to *running* keeberry
//! firmware (VID 0x1209 / PID 0x0001) — neither can reach the **DFU bootloader**
//! (VID 0x342D / PID 0xDFA0), which exposes a USB-DFU interface, not HID. So
//! flashing is native-only and shells out to the bundled `wb32-dfu-updater_cli`
//! `externalBin` sidecar (the QMK-Toolbox / ZSA-Keymapp model).
//!
//! The webview drives this with two commands:
//!
//! * [`reboot_to_normal`] — `wb32-dfu-updater_cli -R`: leave DFU, boot firmware.
//! * [`flash_firmware`]   — wait for the DFU device, write the bundled image at
//!   the flash base, then reset. It streams `flash-progress` events so the UI can
//!   show the round-trip without polling.
//!
//! The sidecar is launched from Rust (never from the webview), so the shell
//! plugin's execute permission is never exposed to the web layer — the ACL gates
//! only webview-initiated shell calls; `Shell::sidecar` from Rust bypasses it.
//!
//! Packaging caveat: the macOS sidecar dynamically links Homebrew's libusb (see
//! `binaries/NOTICE`); that dylib must be bundled or static-linked before the
//! `.app` runs on a clean Mac. It works as-is on a dev machine with `brew`'s
//! libusb. The flash sequence itself is the hardware-verified one; the actual
//! write happens on the user's device.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::path::BaseDirectory;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_shell::process::Output;
use tauri_plugin_shell::ShellExt;

/// The DFU bootloader's USB VID and PID. `wb32-dfu-updater_cli --list` prints them
/// as `Found DFU: [0x342D:0xDFA0]`, so they are matched **separately** (and
/// case-insensitively) against its output — a single `342d:dfa0` substring would
/// miss the `0x` prefixes the tool emits and never match. Parsing the sidecar's
/// text avoids pulling a second libusb into the app to enumerate USB ourselves.
const DFU_VID: &str = "342d";
const DFU_PID: &str = "dfa0";

/// Main-flash origin on the WB32FQ95; the hardware-proven download address.
const FLASH_BASE: &str = "0x08000000";

/// `externalBin` base name (tauri.conf.json `bundle.externalBin`); the shell
/// plugin resolves the target-triple suffix and the dev-vs-bundle location.
const FLASHER_SIDECAR: &str = "wb32-dfu-updater_cli";

/// Bundled `resource` paths, resolved via [`BaseDirectory::Resource`]. They keep
/// the `resources/` prefix the bundler preserves from tauri.conf.json.
const RES_FIRMWARE_BIN: &str = "resources/keeberry.bin";
const RES_FIRMWARE_MANIFEST: &str = "resources/firmware.json";

/// Progress event name the webview listens on.
const EVENT_PROGRESS: &str = "flash-progress";

/// How long to wait for the board to re-enumerate as the DFU device before
/// giving up with an "enter DFU first" error.
const DFU_WAIT_TIMEOUT: Duration = Duration::from_secs(15);
/// Delay between `--list` polls while waiting for the DFU device.
const DFU_POLL_INTERVAL: Duration = Duration::from_millis(300);

/// What the bundled firmware image is — stamped by `stage-firmware.mjs` from
/// `firmware/Cargo.toml` so the app knows the version it ships.
#[derive(Serialize, Deserialize)]
pub struct FirmwareManifest {
    pub version: String,
}

/// One step of the flash round-trip, emitted as a `flash-progress` event.
#[derive(Clone, Serialize)]
struct FlashProgress {
    /// `entering` | `waiting` | `flashing` | `rebooting` | `done` | `error`.
    phase: &'static str,
    /// Human-readable status for the UI.
    message: String,
}

fn emit_progress(app: &AppHandle, phase: &'static str, message: impl Into<String>) {
    let _ = app.emit(
        EVENT_PROGRESS,
        FlashProgress {
            phase,
            message: message.into(),
        },
    );
}

/// Run the flasher sidecar with `args` and collect its output.
async fn run_flasher(app: &AppHandle, args: &[&str]) -> Result<Output, String> {
    app.shell()
        .sidecar(FLASHER_SIDECAR)
        .map_err(|e| format!("could not locate the bundled flasher: {e}"))?
        .args(args.iter().copied())
        .output()
        .await
        .map_err(|e| format!("could not run the flasher: {e}"))
}

/// The sidecar's combined stdout+stderr as a lossy string (the tool prints its
/// device list and errors across both streams).
fn combined_output(out: &Output) -> String {
    let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&out.stderr));
    text
}

/// Format a failed sidecar run for the user, preferring its own message.
fn flasher_error(step: &str, out: &Output) -> String {
    let detail = combined_output(out);
    let detail = detail.trim();
    let code = out
        .status
        .code()
        .map(|c| c.to_string())
        .unwrap_or_else(|| "terminated by signal".to_string());
    if detail.is_empty() {
        format!("flasher {step} failed (exit {code})")
    } else {
        format!("flasher {step} failed (exit {code}): {detail}")
    }
}

/// Is the DFU bootloader present right now? Asks the sidecar to `--list` and
/// looks for our VID:PID; an absent device prints "Not found device!".
async fn dfu_device_present(app: &AppHandle) -> Result<bool, String> {
    let out = run_flasher(app, &["-l"]).await?;
    let text = combined_output(&out).to_ascii_lowercase();
    Ok(text.contains(DFU_VID) && text.contains(DFU_PID))
}

/// Poll until the DFU bootloader appears or the timeout elapses. A timeout is the
/// "you must enter the bootloader first" case, surfaced as a clear error.
async fn wait_for_dfu(app: &AppHandle) -> Result<(), String> {
    let deadline = Instant::now() + DFU_WAIT_TIMEOUT;
    loop {
        if dfu_device_present(app).await? {
            return Ok(());
        }
        if Instant::now() >= deadline {
            return Err(
                "No keyboard found in bootloader (DFU) mode. Put it in the bootloader first \
                 (Enter bootloader / Update firmware), then try again."
                    .to_string(),
            );
        }
        tokio::time::sleep(DFU_POLL_INTERVAL).await;
    }
}

/// SYSTEM round-trip, exit half: `wb32-dfu-updater_cli -R` resets the bootloader
/// so the board leaves DFU and boots the firmware. Only meaningful while the
/// board is in DFU mode; otherwise the tool finds nothing and this says so.
#[tauri::command]
pub async fn reboot_to_normal(app: AppHandle) -> Result<(), String> {
    let out = run_flasher(&app, &["-R"]).await?;
    if out.status.success() {
        return Ok(());
    }
    if combined_output(&out).to_ascii_lowercase().contains("not found") {
        return Err("No keyboard found in bootloader (DFU) mode to reboot.".to_string());
    }
    Err(flasher_error("reset", &out))
}

/// The one-click flash: wait for the DFU device, write the bundled image at the
/// flash base, then reset into the new firmware. Assumes the board is entering
/// (or already in) DFU — the webview sends the kcp ENTER_DFU before calling this,
/// or the user flashes a board already in the bootloader. Progress is streamed as
/// `flash-progress` events; on any failure the final event is `error`.
#[tauri::command]
pub async fn flash_firmware(app: AppHandle) -> Result<(), String> {
    emit_progress(&app, "entering", "Preparing to flash…");

    let bin = app
        .path()
        .resolve(RES_FIRMWARE_BIN, BaseDirectory::Resource)
        .map_err(|e| {
            let msg = format!("could not resolve the bundled firmware image: {e}");
            emit_progress(&app, "error", &msg);
            msg
        })?;
    if !bin.exists() {
        let msg = "Bundled firmware image is missing — this build was not staged with a \
                   firmware image."
            .to_string();
        emit_progress(&app, "error", &msg);
        return Err(msg);
    }
    let bin_path = bin.to_string_lossy().into_owned();

    emit_progress(&app, "waiting", "Waiting for the keyboard's bootloader…");
    if let Err(msg) = wait_for_dfu(&app).await {
        emit_progress(&app, "error", &msg);
        return Err(msg);
    }

    emit_progress(&app, "flashing", "Writing firmware…");
    let out = run_flasher(&app, &["-s", FLASH_BASE, "-D", bin_path.as_str()]).await?;
    if !out.status.success() {
        let msg = flasher_error("download", &out);
        emit_progress(&app, "error", &msg);
        return Err(msg);
    }

    emit_progress(&app, "rebooting", "Rebooting into the new firmware…");
    let out = run_flasher(&app, &["-R"]).await?;
    if !out.status.success() {
        let msg = flasher_error("reset", &out);
        emit_progress(&app, "error", &msg);
        return Err(msg);
    }

    emit_progress(&app, "done", "Firmware updated. The keyboard will reconnect.");
    Ok(())
}

/// Read the bundled firmware manifest (`resources/firmware.json`) so the webview
/// can show the version it ships and compare it with the connected device.
#[tauri::command]
pub fn bundled_firmware(app: AppHandle) -> Result<FirmwareManifest, String> {
    let path = app
        .path()
        .resolve(RES_FIRMWARE_MANIFEST, BaseDirectory::Resource)
        .map_err(|e| format!("could not resolve the firmware manifest: {e}"))?;
    let text = std::fs::read_to_string(&path)
        .map_err(|e| format!("bundled firmware manifest unavailable: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("invalid firmware manifest: {e}"))
}
