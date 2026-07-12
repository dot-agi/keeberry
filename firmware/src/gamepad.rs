// SPDX-License-Identifier: GPL-2.0-or-later
//! USB HID gamepad keys: a stateless DirectInput-style joystick decoder.
//!
//! keeberry has no analog stick, but the keymap can bind the [`GamepadKey`]
//! actions ([`crate::keycode`]): sixteen momentary buttons and four signed axes
//! (left stick X/Y, right stick Z/Rz), each axis driven by a pair of keys. They
//! carry no keyboard usage, so [`crate::keymap::compute_report`] ignores them;
//! instead [`crate::keymap::gamepad_keys`] resolves the *currently held* gamepad
//! keys each scan into the compact [bitmask](self#bitmask) this module decodes,
//! and the shared report-ID interface ([`crate::usb`]) sends gamepad HID reports
//! (report ID 5) built from it — no new USB endpoint, the gamepad report rides EP3
//! alongside NKRO, consumer, system control and the mouse.
//!
//! # Why no accelerator (unlike the mouse)
//!
//! A HID gamepad axis is *absolute*: each report states the stick's position, so a
//! digital key holding a direction simply pins that axis to full deflection and
//! releasing it returns the axis to centre. There is no per-report delta to ramp
//! (contrast [`crate::mouse`], whose *relative* pointer needs an accelerator), so
//! this module is pure: the held-key bitmask maps directly to the report bytes with
//! no timing state. The (potentially host-blocked) EP3 write still happens in
//! [`crate::usb`]'s send loop, off the matrix-scan path, exactly like the mouse.
//!
//! # Bitmask
//!
//! The held gamepad keys are carried as a `u32`: bits `0..=15` are the sixteen
//! buttons (bit `n` = button `n`, HID Button usage `n + 1`), bits `16..=23` are the
//! eight axis-direction flags below (one per signed end of the four axes).
//! [`crate::keymap::gamepad_keys`] builds it from the matrix; [`buttons`] and
//! [`axes`] decode it.
//!
//! # Buttons vs. axes
//!
//! Both buttons and axes are *absolute* HID state (a held button bit, a stick
//! position), so the whole report is level state and a release must be reported —
//! the send loop dedups the complete report against the last one it sent rather
//! than tracking edges. Holding the two keys of one axis at once cancels to centre
//! ([`axis`]), the SOCD-style mutual cancel that keeps e.g. left+right from pinning
//! the stick to one side.
//!
//! Scope: USB only. There is no vendor radio frame for a gamepad, so on a wireless
//! transport the held gamepad keys simply do not emit (the send loop skips the
//! gamepad report and clears its cache on the switch), mirroring the mouse.

// === Held-gamepad-key bitmask ===============================================
//
// Buttons occupy the low sixteen bits (button `n` -> bit `n`); the eight axis
// flags sit above them. Keeping buttons in a contiguous low field lets [`buttons`]
// extract them with a single mask, and the report's 16-bit button field is just
// those bits in little-endian order.

/// Number of gamepad buttons (HID Button usages `1..=16`), the width of the
/// button field in the report and the low bits of the held-key bitmask.
pub const BUTTON_COUNT: u8 = 16;

/// Left-stick X axis held to its negative end (−127).
pub const X_NEG: u32 = 1 << 16;
/// Left-stick X axis held to its positive end (+127).
pub const X_POS: u32 = 1 << 17;
/// Left-stick Y axis held negative (−127).
pub const Y_NEG: u32 = 1 << 18;
/// Left-stick Y axis held positive (+127).
pub const Y_POS: u32 = 1 << 19;
/// Right-stick Z axis held negative (−127).
pub const Z_NEG: u32 = 1 << 20;
/// Right-stick Z axis held positive (+127).
pub const Z_POS: u32 = 1 << 21;
/// Right-stick Rz axis held negative (−127).
pub const RZ_NEG: u32 = 1 << 22;
/// Right-stick Rz axis held positive (+127).
pub const RZ_POS: u32 = 1 << 23;

/// Mask selecting the sixteen button bits out of the held-key bitmask.
const BUTTON_MASK: u32 = (1 << BUTTON_COUNT) - 1;

/// Full axis deflection a held direction key drives, well inside a signed byte and
/// matching the report's Logical Maximum (`+127`).
const AXIS_MAX: i8 = 127;
/// The opposing full deflection (the report's Logical Minimum, `−127`).
const AXIS_MIN: i8 = -127;

/// The 16-bit HID button field from the held-key bitmask: bit `n` = button `n + 1`,
/// the standard ascending Button-usage layout. Sent little-endian (buttons `1..=8`
/// in the low byte) in the report.
pub fn buttons(keys: u32) -> u16 {
    (keys & BUTTON_MASK) as u16
}

/// The four signed axis bytes `[X, Y, Z, Rz]` from the held-key bitmask, each at
/// full deflection (`±127`) when exactly one of its direction keys is held and
/// centred (`0`) otherwise — see [`axis`].
pub fn axes(keys: u32) -> [i8; 4] {
    [
        axis(keys, X_POS, X_NEG),
        axis(keys, Y_POS, Y_NEG),
        axis(keys, Z_POS, Z_NEG),
        axis(keys, RZ_POS, RZ_NEG),
    ]
}

/// One axis byte from its two opposing held flags: `+127` positive only, `−127`
/// negative only, `0` if neither or both — the SOCD-style mutual cancel that keeps
/// e.g. left+right from pinning the stick to one side.
fn axis(keys: u32, positive: u32, negative: u32) -> i8 {
    match (keys & positive != 0, keys & negative != 0) {
        (true, false) => AXIS_MAX,
        (false, true) => AXIS_MIN,
        _ => 0,
    }
}
