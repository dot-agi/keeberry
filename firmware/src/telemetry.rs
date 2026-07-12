// SPDX-License-Identifier: GPL-2.0-or-later
//! Device telemetry: lock-free counters and timings sampled by the keyboard
//! loop and read back over the kcp TELEMETRY group.
//!
//! The hot path ([`crate::usb::keyboard_loop`]) updates these statics every
//! millisecond; the kcp config loop reads them when answering
//! [`crate::kcp::CMD_GET_TELEMETRY`]. Both run as cooperative futures on the one
//! thread-mode executor and each field is an independent counter with no
//! cross-field invariant, so a single-core [`Ordering::Relaxed`] atomic per
//! field is all the synchronisation needed: there is nothing to lock and no torn
//! read to guard against. Uptime is deliberately *not* stored here — it is read
//! straight from the monotonic [`embassy_time::Instant::now`] when the host asks,
//! so it needs no periodic update and cannot drift.
//!
//! Each field exposes a writer (`inc_*` / `set_*` / `record_*`) used by the
//! keyboard loop and a getter used by the telemetry handler; see the items.

use core::sync::atomic::{AtomicU16, AtomicU32, Ordering};
use embassy_time::Duration;

/// Total matrix scans since boot — one per [`crate::usb::keyboard_loop`]
/// iteration. Monotonic; wraps after `2^32` scans (~49.7 days at the 1 kHz loop).
static SCAN_COUNT: AtomicU32 = AtomicU32::new(0);

/// Total HID keyboard reports actually written to the IN endpoint since boot.
/// Only *changed* reports are sent, so this tracks key activity rather than the
/// scan rate. Monotonic; wraps after `2^32` reports.
static REPORT_COUNT: AtomicU32 = AtomicU32::new(0);

/// Most recently computed active-layer bitmask (bit `n` = layer `n`), mirrored
/// from [`crate::keymap::LayerState::active`]. Initialised to `1` (only the base
/// layer active) — the keymap engine's own starting state — so the value is
/// meaningful even before the first scan records one.
static ACTIVE_LAYERS: AtomicU16 = AtomicU16::new(1);

/// Processing time of the last keyboard-loop iteration, in microseconds: matrix
/// scan + debounce + [`crate::keymap::compute_report`]. A real firmware-latency
/// figure — it excludes the 1 ms inter-scan delay and the USB transfer. Zero
/// until the first iteration records one.
static LAST_PROC_US: AtomicU32 = AtomicU32::new(0);

/// Count one completed matrix scan. Called once per keyboard-loop iteration.
pub fn inc_scan() {
    SCAN_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Count one HID keyboard report written to the host.
pub fn inc_report() {
    REPORT_COUNT.fetch_add(1, Ordering::Relaxed);
}

/// Record the active-layer bitmask computed by the latest scan.
pub fn set_active_layers(mask: u16) {
    ACTIVE_LAYERS.store(mask, Ordering::Relaxed);
}

/// Record the processing time of the latest keyboard-loop iteration.
///
/// `elapsed` is [`Instant::elapsed`](embassy_time::Instant::elapsed) measured
/// across scan + debounce + keymap. It is stored as whole microseconds; the
/// `u64 -> u32` narrowing is lossless for any plausible iteration, since `u32`
/// microseconds spans ~71 minutes — far beyond a sub-millisecond scan.
pub fn record_proc(elapsed: Duration) {
    LAST_PROC_US.store(elapsed.as_micros() as u32, Ordering::Relaxed);
}

/// Total matrix scans since boot. See [`SCAN_COUNT`].
pub fn scan_count() -> u32 {
    SCAN_COUNT.load(Ordering::Relaxed)
}

/// Total HID keyboard reports written since boot. See [`REPORT_COUNT`].
pub fn report_count() -> u32 {
    REPORT_COUNT.load(Ordering::Relaxed)
}

/// Most recently computed active-layer bitmask. See [`ACTIVE_LAYERS`].
pub fn active_layers() -> u16 {
    ACTIVE_LAYERS.load(Ordering::Relaxed)
}

/// Processing time of the last keyboard-loop iteration, in microseconds. See
/// [`LAST_PROC_US`].
pub fn last_proc_us() -> u32 {
    LAST_PROC_US.load(Ordering::Relaxed)
}
