// SPDX-License-Identifier: GPL-2.0-or-later
//! Rust HID bridge — the native [`Transport`] for the kcp web app.
//!
//! WKWebView (the macOS webview Tauri uses) has no WebHID, so the React app cannot
//! reach the keyboard directly the way it does in a browser. Instead it calls these
//! four commands and listens for one event:
//!
//! * `hid_list`  — enumerate the keeberry kcp interface (VID/PID + usage page).
//! * `hid_open`  — open one device and start a worker thread that reads inbound
//!                 32-byte reports and emits each as a `kcp-report` event.
//! * `hid_write` — queue one 32-byte OUT report (the worker prepends report-id 0).
//! * `hid_close` — stop the worker and release the device.
//!
//! hidapi's `HidDevice` is `Send` but not `Sync`, so the open device lives on a
//! single worker thread that owns it exclusively: it polls the device for inbound
//! reports and drains a channel of outbound ones. The commands never touch the
//! device pointer — they talk to the worker through the channel and a stop flag —
//! which keeps all HID I/O on one thread without sharing the handle.

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use hidapi::{HidApi, HidDevice};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

/// The running keeberry firmware's USB identity.
const KEEBERRY_VID: u16 = 0x1209;
const KEEBERRY_PID: u16 = 0x0001;
/// The stock Akko 2.4 GHz dongle's USB identity. It bridges the kcp interface over
/// the radio but keeps its own Akko VID/PID even while carrying a keeberry keyboard
/// (verified on hardware), so it must be recognized explicitly for the configurator
/// to reach the keyboard over 2.4 GHz.
const AKKO_DONGLE_VID: u16 = 0x342d;
const AKKO_DONGLE_PID: u16 = 0xe4d7;
/// The USB identities that carry the kcp interface — the keeberry keyboard direct,
/// or the Akko dongle bridging it over 2.4 GHz. Every match is additionally pinned
/// to the kcp usage page/usage below, so unrelated `0xFF60` boards are not matched.
const KCP_DEVICE_IDS: [(u16, u16); 2] =
    [(KEEBERRY_VID, KEEBERRY_PID), (AKKO_DONGLE_VID, AKKO_DONGLE_PID)];
/// The kcp raw-HID vendor interface (QMK-style usage page / usage).
const KCP_USAGE_PAGE: u16 = 0xff60;
const KCP_USAGE: u16 = 0x61;

/// One kcp message is exactly one 32-byte HID report (IN and OUT).
const MSG_LEN: usize = 32;
/// Worker read-poll interval. hidapi cannot interrupt a blocking read, so the
/// worker polls at ~60 Hz: a queued OUT report and `hid_close` are both serviced
/// within this window, at negligible idle cost.
const READ_POLL_MS: i32 = 16;

/// Inbound report event: payload is the raw 32 bytes of one IN frame.
const EVENT_REPORT: &str = "kcp-report";
/// Disconnect event: the device left the bus (unplug / MCU reset).
const EVENT_DISCONNECT: &str = "kcp-disconnect";

/// One enumerated kcp device, as handed to the webview for selection.
#[derive(Serialize)]
pub struct DeviceInfo {
    /// Opaque OS path used to reopen this exact interface in `hid_open`.
    path: String,
    /// Human-friendly name for the app-rendered picker.
    name: String,
}

/// The Tauri-managed HID state: a cached enumerator and the lone open device's
/// worker, behind one mutex. The commands are short critical sections; the worker
/// never locks this, so it cannot contend with or deadlock the command thread.
#[derive(Default)]
pub struct HidState {
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    /// Created lazily and kept alive for the process: re-enumerating is cheap and
    /// this sidesteps repeated global hidapi init.
    api: Option<HidApi>,
    worker: Option<Worker>,
}

/// Handle to the thread that owns the open device.
struct Worker {
    /// Outbound 33-byte frames ([report-id 0, ..32]) to write to the device.
    tx: Sender<Vec<u8>>,
    /// Set to ask the worker to exit; distinguishes an intentional close from a
    /// genuine disconnect so the latter is the only one that emits.
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Lazily create (or reuse) the cached hidapi context.
fn ensure_api(inner: &mut Inner) -> Result<&mut HidApi, String> {
    let api = match inner.api {
        Some(ref mut a) => a,
        None => inner.api.insert(HidApi::new().map_err(|e| e.to_string())?),
    };
    Ok(api)
}

/// Stop and join the current worker, if any, releasing its device.
fn stop_worker(inner: &mut Inner) {
    if let Some(worker) = inner.worker.take() {
        worker.stop.store(true, Ordering::SeqCst);
        // The worker checks `stop` each poll and exits within READ_POLL_MS; join
        // so the device is fully released before we return (or reopen).
        let _ = worker.handle.join();
    }
}

/// The worker loop: own `device`, drain queued writes, poll for inbound reports,
/// and translate the device leaving the bus into a disconnect event.
fn spawn_worker(
    device: HidDevice,
    rx: Receiver<Vec<u8>>,
    stop: Arc<AtomicBool>,
    app: AppHandle,
) -> JoinHandle<()> {
    thread::spawn(move || {
        let mut buf = [0u8; MSG_LEN];
        loop {
            if stop.load(Ordering::SeqCst) {
                return;
            }
            // Flush queued OUT reports first, so a freshly-issued request goes out
            // before we block on the next read.
            loop {
                match rx.try_recv() {
                    Ok(frame) => {
                        if device.write(&frame).is_err() {
                            if !stop.load(Ordering::SeqCst) {
                                let _ = app.emit(EVENT_DISCONNECT, ());
                            }
                            return;
                        }
                    }
                    Err(TryRecvError::Empty) => break,
                    // hid_close dropped the sender: an intentional close.
                    Err(TryRecvError::Disconnected) => return,
                }
            }
            match device.read_timeout(&mut buf, READ_POLL_MS) {
                Ok(0) => {}
                Ok(n) => {
                    let _ = app.emit(EVENT_REPORT, buf[..n].to_vec());
                }
                Err(_) => {
                    // A read error means the device left the bus (unplug / reset),
                    // unless we asked it to stop.
                    if !stop.load(Ordering::SeqCst) {
                        let _ = app.emit(EVENT_DISCONNECT, ());
                    }
                    return;
                }
            }
        }
    })
}

/// Enumerate the kcp interface — the keeberry keyboard (0x1209/0x0001) or the Akko
/// 2.4 GHz dongle bridging it (0x342d/0xe4d7) — on usage page 0xFF60 (usage 0x61).
/// On macOS hidapi yields one entry per top-level collection, so this returns the
/// openable path of the vendor interface specifically.
#[tauri::command]
pub fn hid_list(state: State<HidState>) -> Result<Vec<DeviceInfo>, String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "HID state poisoned".to_string())?;
    let api = ensure_api(&mut inner)?;
    api.refresh_devices().map_err(|e| e.to_string())?;
    let devices = api
        .device_list()
        .filter(|d| {
            KCP_DEVICE_IDS.contains(&(d.vendor_id(), d.product_id()))
                && d.usage_page() == KCP_USAGE_PAGE
                && d.usage() == KCP_USAGE
        })
        .map(|d| DeviceInfo {
            path: d.path().to_string_lossy().into_owned(),
            name: d.product_string().unwrap_or("keeberry device").to_string(),
        })
        .collect();
    Ok(devices)
}

/// Open the device at `path` and start its worker thread. Replaces any device that
/// was already open.
#[tauri::command]
pub fn hid_open(path: String, app: AppHandle, state: State<HidState>) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "HID state poisoned".to_string())?;
    stop_worker(&mut inner);

    let cpath = CString::new(path).map_err(|_| "device path contains a NUL byte".to_string())?;
    let device = ensure_api(&mut inner)?
        .open_path(&cpath)
        .map_err(|e| e.to_string())?;

    let stop = Arc::new(AtomicBool::new(false));
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    let handle = spawn_worker(device, rx, stop.clone(), app);
    inner.worker = Some(Worker { tx, stop, handle });
    Ok(())
}

/// Queue one 32-byte OUT report. hidapi expects a leading report-id byte and the
/// kcp interface uses none (report id 0), so the worker writes 33 bytes:
/// `[0x00, ..32]`.
#[tauri::command]
pub fn hid_write(bytes: Vec<u8>, state: State<HidState>) -> Result<(), String> {
    let inner = state
        .inner
        .lock()
        .map_err(|_| "HID state poisoned".to_string())?;
    let worker = inner
        .worker
        .as_ref()
        .ok_or_else(|| "no keeberry device is open".to_string())?;

    let mut frame = Vec::with_capacity(MSG_LEN + 1);
    frame.push(0);
    frame.extend_from_slice(&bytes);
    // Fix the OUT report at 33 bytes (report-id + 32 payload), padding or clamping
    // a short/long write to the wire length the firmware expects.
    frame.resize(MSG_LEN + 1, 0);

    worker
        .tx
        .send(frame)
        .map_err(|_| "keeberry device worker has stopped".to_string())
}

/// Stop the worker and release the device. A no-op if nothing is open.
#[tauri::command]
pub fn hid_close(state: State<HidState>) -> Result<(), String> {
    let mut inner = state
        .inner
        .lock()
        .map_err(|_| "HID state poisoned".to_string())?;
    stop_worker(&mut inner);
    Ok(())
}
