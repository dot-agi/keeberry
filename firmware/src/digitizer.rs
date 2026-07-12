// SPDX-License-Identifier: GPL-2.0-or-later
//! Absolute-pointer (HID digitizer) state for the shared report-ID interface.
//!
//! The digitizer is a *host-driven* absolute pointer: a single-touch contact
//! whose position and tip/in-range state are set by the host over the kcp
//! [`SYSTEM.SET_DIGITIZER`](crate::kcp::CMD_SYSTEM_SET_DIGITIZER) command rather
//! than by the key matrix (the keymap owns no digitizer keycode). Because it is a
//! plain HID collection on usage page `0x0D` it rides the existing shared EP3
//! interface as one more report ID — exactly like the mouse and gamepad — so it
//! costs **no new endpoint** and needs no re-enumerated USB mode (unlike MIDI /
//! XInput). [`crate::usb`] owns the wire descriptor and emits the report; this
//! module owns the runtime contact state and packs the report payload, mirroring
//! how [`crate::mouse`] / [`crate::gamepad`] own their report-building logic.
//!
//! The intended use is host/automation test control of an absolute pointer (e.g.
//! driving the cursor to known coordinates), not an end-user input path.

use core::sync::atomic::{AtomicU32, Ordering};

/// Digitizer report ID on the shared interface (after NKRO=1 … gamepad=5).
pub const REPORT_ID: u8 = 6;

/// Inclusive maximum of the absolute X/Y logical range (15 bits, `0x7FFF`).
/// Matches the descriptor's `Logical Maximum (32767)` in [`crate::usb`]; the host
/// scales its own coordinate space onto `0..=LOGICAL_MAX` before sending.
pub const LOGICAL_MAX: u16 = 0x7FFF;

/// Wire length of the digitizer report: report ID + a status byte (tip switch +
/// in-range) + absolute X (`u16` LE) + absolute Y (`u16` LE).
pub const REPORT_LEN: usize = 6;

// Packed contact state: X in bits 0..15, Y in bits 15..30, tip switch in bit 30,
// in-range in bit 31. A single `u32` so the host-set position and the read in the
// EP3 send loop are one relaxed atomic store/load — the same single-producer
// (kcp dispatch) / single-consumer (shared EP loop) hand-off the consumer/mouse/
// gamepad state uses, with no torn read between the X and Y halves.
const X_SHIFT: u32 = 0;
const Y_SHIFT: u32 = 15;
const TIP_BIT: u32 = 1 << 30;
const IN_RANGE_BIT: u32 = 1 << 31;
const COORD_MASK: u32 = 0x7FFF;

/// The last contact state the host set. Idle (origin, tip up, out of range) at
/// power-on, so nothing is reported until the host drives it.
static STATE: AtomicU32 = AtomicU32::new(0);

/// Set the absolute contact state (the kcp `SYSTEM.SET_DIGITIZER` op). `x`/`y` are
/// clamped to `0..=LOGICAL_MAX`; `tip` is the tip-switch (touching) flag and
/// `in_range` whether the pointer is in sensing range. Applied on the next
/// [`crate::usb`] shared-EP send.
pub fn set(x: u16, y: u16, tip: bool, in_range: bool) {
    let x = x.min(LOGICAL_MAX) as u32;
    let y = y.min(LOGICAL_MAX) as u32;
    let mut packed = (x << X_SHIFT) | (y << Y_SHIFT);
    if tip {
        packed |= TIP_BIT;
    }
    if in_range {
        packed |= IN_RANGE_BIT;
    }
    STATE.store(packed, Ordering::Relaxed);
}

/// Pack the current contact into its HID report (report ID + status + X + Y).
///
/// The status byte's bit 0 is the tip switch and bit 1 is in-range, matching the
/// descriptor's two 1-bit fields followed by 6 padding bits; X and Y are
/// little-endian `u16`. Returned by value so the caller dedups it against the last
/// report it sent (the whole contact is absolute state).
pub fn report() -> [u8; REPORT_LEN] {
    let packed = STATE.load(Ordering::Relaxed);
    let x = ((packed >> X_SHIFT) & COORD_MASK) as u16;
    let y = ((packed >> Y_SHIFT) & COORD_MASK) as u16;
    let mut status = 0u8;
    if packed & TIP_BIT != 0 {
        status |= 0x01;
    }
    if packed & IN_RANGE_BIT != 0 {
        status |= 0x02;
    }
    let [x_lo, x_hi] = x.to_le_bytes();
    let [y_lo, y_hi] = y.to_le_bytes();
    [REPORT_ID, status, x_lo, x_hi, y_lo, y_hi]
}

/// The idle digitizer report: origin, tip up, out of range. The shared-EP send
/// loop seeds and resyncs its changed-only cache to this so a host-set contact
/// still in effect across a (re)configuration is re-sent rather than masked.
pub const IDLE: [u8; REPORT_LEN] = [REPORT_ID, 0, 0, 0, 0, 0];
