// SPDX-License-Identifier: GPL-2.0-or-later
//! XInput (Xbox 360 controller) mode: the key matrix as a gamepad.
//!
//! XInput is a vendor class (interface `0xFF`/`0x5D`/`0x01` under VID/PID
//! `045E:028E`), not HID, so — like [MIDI](crate::midi) — it needs its own
//! interrupt endpoints and cannot share the spent EP3 HID interface. It therefore
//! lives behind a *re-enumerated* USB mode ([`crate::usb::UsbMode::Xinput`]): the
//! keyboard detaches and rebuilds its descriptor as an Xbox 360 controller (one
//! interrupt IN + one interrupt OUT) alongside the kcp control interface, then
//! runs this loop. Entering / leaving is the kcp
//! [`SYSTEM.SET_USB_MODE`](crate::kcp::CMD_SYSTEM_SET_USB_MODE) command.
//!
//! The descriptor shape (the `045E:028E` identity, the magic vendor descriptor
//! and the `0x81`/`0x01` endpoint addresses it names) follows the wired
//! Xbox 360 controller and the `em-usb-pad` prior art
//! (<https://github.com/hyx0329/em-usb-pad>), which is what the Microsoft XUSB
//! driver binds against.
//!
//! # Mapping (built-in)
//!
//! The matrix is read row-major; the first [`BUTTON_BITS`]`.len()` keys map, in
//! order, to the controller's digital buttons (D-pad, Start/Back, stick clicks,
//! bumpers, Guide, A/B/X/Y). Triggers and sticks are left centred — the switches
//! are digital, so only the button field is driven. Keys past the button budget
//! do nothing.
//!
//! # Leaving XInput
//!
//! Once a host OS recognises the Xbox 360 controller (`045E:028E`) it claims the
//! *whole* device — the kcp control interface included — so the host can no longer
//! command a switch back: kcp `SET_USB_MODE` cannot reach the device on Windows,
//! Linux or macOS. As a firmware-side escape, holding **Fn + Right Ctrl** (the two
//! adjacent bottom-right keys in the default layout) together for [`ESCAPE_HOLD`]
//! requests [`UsbMode::Normal`](crate::usb::UsbMode::Normal) and re-enumerates the
//! keeberry composite (see [`run`]). The whole panel glows solid red while both keys
//! are held, so the combo is visibly registering; the two keys sit past the
//! gamepad-button budget, so it never doubles as a controller button. (An
//! unplug/replug also returns to Normal, since the mode is not persisted.)

use crate::matrix::{self, NUM_COLS, NUM_ROWS};
use embassy_futures::select::select3;
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::driver::{Driver, EndpointIn, EndpointOut};
use embassy_usb::Builder;

/// Xbox 360 wired controller USB identity. The XUSB driver binds on this VID/PID
/// together with the vendor interface triple below, so the mode advertises them
/// for the duration of the re-enumeration (the keeberry identity returns on exit).
pub const VID: u16 = 0x045E;
pub const PID: u16 = 0x028E;

/// Vendor interface triple for the XInput control interface (class `0xFF`,
/// subclass `0x5D` "Xbox", protocol `0x01`).
pub const IF_CLASS: u8 = 0xFF;
pub const IF_SUBCLASS: u8 = 0x5D;
pub const IF_PROTOCOL: u8 = 0x01;

/// The XInput "magic" vendor descriptor (`bDescriptorType = 0x21`). [`Builder`]'s
/// `descriptor` prepends `bLength` and `bDescriptorType`, so this is the 15-byte
/// body only; on the wire it becomes the canonical 17-byte
/// `11 21 00 01 01 25 81 14 00 00 00 00 13 01 08 00 00`. The XUSB driver parses it
/// to find the controller's interrupt endpoints, so four on-wire bytes are
/// load-bearing and must match what [`build`] allocates: byte 6 (`0x81`) is the IN
/// endpoint address, byte 7 (`0x14`) the IN report size (20), byte 13 (`0x01`) the
/// OUT endpoint address and byte 14 (`0x08`) the OUT report size (8). The endpoints
/// are auto-allocated — the same path the working [MIDI](crate::midi) mode takes —
/// and, because XInput is built first, land at exactly the `0x81` IN / `0x01` OUT
/// (both on EP1) the descriptor names.
const MAGIC_DESCRIPTOR_TYPE: u8 = 0x21;
const MAGIC_DESCRIPTOR: &[u8] = &[
    0x00, 0x01, 0x01, 0x25, 0x81, 0x14, 0x00, 0x00, 0x00, 0x00, 0x13, 0x01, 0x08, 0x00, 0x00,
];

/// Input report size (the fixed 20-byte Xbox 360 input report).
pub const REPORT_LEN: usize = 20;
/// Interrupt endpoint max-packet size (`wMaxPacketSize`), used for both directions.
/// The real Xbox 360 controller uses 32-byte interrupt endpoints; the 20-byte input
/// report ([`REPORT_LEN`]) rides the IN endpoint within this. 32 rather than 20
/// because a full-speed interrupt endpoint's max-packet size must be a multiple of 8,
/// which 20 is not.
const EP_MAX_PACKET: u16 = 32;
/// Input poll interval (ms) — the controller's 4 ms reporting rate.
const IN_INTERVAL_MS: u8 = 4;
/// Output poll interval (ms).
const OUT_INTERVAL_MS: u8 = 8;

/// Button bit positions within the 16-bit button field (`report[2..4]`, LE),
/// skipping the reserved bit 11. In order: D-pad up/down/left/right, Start, Back,
/// left-stick click, right-stick click, LB, RB, Guide, A, B, X, Y. The matrix's
/// first 15 keys (row-major) map onto these in sequence.
const BUTTON_BITS: [u8; 15] = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12, 13, 14, 15];

/// Build the XInput interface on `builder`: the magic vendor descriptor and the
/// auto-allocated interrupt IN/OUT endpoints. Returns both endpoints for [`run`] to
/// drive — the IN carries the gamepad input reports, the OUT is drained of the host's
/// rumble/LED output reports so they cannot back up the pipe.
pub fn build<'d, D: Driver<'d>>(builder: &mut Builder<'d, D>) -> (D::EndpointIn, D::EndpointOut) {
    let mut func = builder.function(IF_CLASS, IF_SUBCLASS, IF_PROTOCOL);
    let mut iface = func.interface();
    let mut alt = iface.alt_setting(IF_CLASS, IF_SUBCLASS, IF_PROTOCOL, None);
    alt.descriptor(MAGIC_DESCRIPTOR_TYPE, MAGIC_DESCRIPTOR);
    let ep_in = alt.endpoint_interrupt_in(None, EP_MAX_PACKET, IN_INTERVAL_MS);
    let ep_out = alt.endpoint_interrupt_out(None, EP_MAX_PACKET, OUT_INTERVAL_MS);
    (ep_in, ep_out)
}

/// Pack the held matrix keys into a 20-byte Xbox 360 input report. Byte 0/1 are
/// the report type/length header; bytes 2..4 the button field; triggers and the
/// two sticks (bytes 4..14) stay centred; bytes 14..20 are reserved.
fn report(scan: &[u16; NUM_ROWS]) -> [u8; REPORT_LEN] {
    let mut buttons: u16 = 0;
    let mut key_index = 0usize;
    for row in scan.iter() {
        for col in 0..NUM_COLS {
            if key_index >= BUTTON_BITS.len() {
                break;
            }
            if row & (1 << col) != 0 {
                buttons |= 1 << BUTTON_BITS[key_index];
            }
            key_index += 1;
        }
    }
    let mut buf = [0u8; REPORT_LEN];
    buf[0] = 0x00; // message type: input report
    buf[1] = REPORT_LEN as u8; // message length
    buf[2] = buttons as u8;
    buf[3] = (buttons >> 8) as u8;
    buf
}

/// Escape combo, first key: **Fn** — the `MO(1)` momentary-layer key at matrix
/// `(row 5, col 10)` in the default layout. Held together with [`ESCAPE_KEY_RCTL`]
/// to leave XInput (see [`run`]). Chosen over the old Left Ctrl + Right Arrow pair
/// because that doubled as the macOS "move space right" shortcut; Fn + Right Ctrl is
/// not a system gesture on any host.
const ESCAPE_KEY_FN: (usize, usize) = (5, 10);
/// Escape combo, second key: **Right Ctrl** at matrix `(row 5, col 11)`, immediately
/// right of [`ESCAPE_KEY_FN`] in the bottom-right cluster. Both are real keys past
/// the gamepad-button budget, so holding the pair emits no spurious controller button.
const ESCAPE_KEY_RCTL: (usize, usize) = (5, 11);

/// How long both escape keys must be held *continuously* before XInput is left.
/// Long enough that no incidental brush of both keys triggers it, short enough to be
/// practical to perform deliberately — and the window over which the red feedback
/// override is shown.
const ESCAPE_HOLD: Duration = Duration::from_millis(1500);

// The escape keys must stay within the matrix (so the scan lookup is in range) and
// past the gamepad-button budget — their row-major index is >= BUTTON_BITS.len() — so
// holding the combo can never also emit a controller button (only the first
// BUTTON_BITS.len() row-major keys map to buttons; see `report`).
const _: () = assert!(
    ESCAPE_KEY_FN.0 < NUM_ROWS
        && ESCAPE_KEY_RCTL.0 < NUM_ROWS
        && ESCAPE_KEY_FN.1 < NUM_COLS
        && ESCAPE_KEY_RCTL.1 < NUM_COLS
        && ESCAPE_KEY_FN.0 * NUM_COLS + ESCAPE_KEY_FN.1 >= BUTTON_BITS.len()
        && ESCAPE_KEY_RCTL.0 * NUM_COLS + ESCAPE_KEY_RCTL.1 >= BUTTON_BITS.len(),
    "escape keys must be within the matrix and past the gamepad-button budget"
);

/// Whether both escape keys (see [`run`]) are held in `scan` (read from the raw
/// matrix, so no debouncing gates the combo).
fn escape_held(scan: &[u16; NUM_ROWS]) -> bool {
    let held = |(row, col): (usize, usize)| scan[row] & (1u16 << col) != 0;
    held(ESCAPE_KEY_FN) && held(ESCAPE_KEY_RCTL)
}

/// Escape-feedback colour painted while both keys are held: pure red (hue `0`) at
/// full saturation and value. With [`crate::rgb::MODE_SOLID`] this lights the whole
/// panel red — the visible "both keys registered" signal the silent old escape lacked.
const ESCAPE_FEEDBACK_HUE: u8 = 0;
const ESCAPE_FEEDBACK_SAT: u8 = 255;
const ESCAPE_FEEDBACK_VAL: u8 = 255;
/// Master brightness for the escape feedback — full, so the red is unmistakable (the
/// RGB renderer still clamps emission to the panel's safe current cap).
const ESCAPE_FEEDBACK_BRIGHTNESS: u8 = 255;

/// An in-progress escape hold: when the combo became fully held, plus the live RGB
/// effect saved at that instant so the red feedback override can be undone if the keys
/// lift before [`ESCAPE_HOLD`] elapses.
///
/// The override is written through the shared [`crate::rgb`] state, which the
/// independent [`rgb_task`](crate::rgb::rgb_task) renders every frame regardless of
/// USB mode — so it is visible in XInput without this loop touching the LEDs itself.
struct EscapeHold {
    /// When the combo first became fully held; the hold fires once [`ESCAPE_HOLD`]
    /// has elapsed from here.
    since: Instant,
    /// Effect mode active before the override, restored on an early release.
    mode: u8,
    /// Effect colour `(h, s, v)` active before the override.
    hsv: (u8, u8, u8),
    /// Master brightness active before the override.
    brightness: u8,
}

impl EscapeHold {
    /// Begin a hold: stamp the start, snapshot the live RGB effect, then override to a
    /// bright solid red so both keys are seen to register.
    fn begin() -> Self {
        let saved = Self {
            since: Instant::now(),
            mode: crate::rgb::mode(),
            hsv: crate::rgb::hsv(),
            brightness: crate::rgb::brightness(),
        };
        // MODE_SOLID is always a valid id, so the `set_mode` result is not meaningful.
        let _ = crate::rgb::set_mode(crate::rgb::MODE_SOLID);
        crate::rgb::set_hsv(ESCAPE_FEEDBACK_HUE, ESCAPE_FEEDBACK_SAT, ESCAPE_FEEDBACK_VAL);
        crate::rgb::set_brightness(ESCAPE_FEEDBACK_BRIGHTNESS);
        saved
    }

    /// Restore the effect snapshotted by [`begin`](Self::begin): the combo was released
    /// before completing, so the red override is undone (a brush never recolours the
    /// panel permanently). On a *completed* hold this is not called — the re-enumeration
    /// to Normal rebuilds RGB from the persisted config instead.
    fn restore(self) {
        // The saved mode came from `rgb::mode()`, so it is always valid to set back.
        let _ = crate::rgb::set_mode(self.mode);
        let (h, s, v) = self.hsv;
        crate::rgb::set_hsv(h, s, v);
        crate::rgb::set_brightness(self.brightness);
    }
}

/// Drive the XInput device from the key matrix until the mode is left.
///
/// Three concurrent blocks race under [`select3`], with the matrix scan deliberately
/// **decoupled** from the (blocking) endpoint write so the escape combo is honoured no
/// matter how the host treats the IN endpoint. macOS recognises the `045E:028E`
/// controller but does not actively poll its vendor IN endpoint the way a real XUSB
/// driver does; were scanning and sending in one loop, that loop would park forever on
/// the first [`write`](EndpointIn::write) and starve the escape watch — the failure
/// this split fixes. The escape therefore lives in a block that touches no endpoint.
///
/// - **scanner** — the sole caller of [`matrix::scan`]; owns the
///   [`matrix::Debouncer`] and the escape-hold clock. Every ~1 ms it reads the raw
///   matrix, evaluates the escape combo on that *raw* scan (a 1.5 s hold needs no
///   debouncing, so the debouncer is taken out as a variable in whether the escape
///   fires), then debounces the same scan for the packed gamepad report and publishes
///   that report into a shared [`Cell`](core::cell::Cell). It holds no endpoint, so it
///   runs at a steady rate independent of the host, and is the one block that can
///   return — which it does when the escape fires, ending `run`.
/// - **sender** — waits for the host to enable the IN endpoint, then writes the
///   scanner's latest report whenever it changes. It never scans, so a write that
///   blocks because the host has stopped draining can no longer stall the scanner or
///   the escape watch. A failed write (disconnect / mode left) drops back to waiting
///   for the endpoint.
/// - **drain** — reads and discards the host's rumble / LED-ring output reports. The
///   endpoint must be serviced or those writes back up and NAK; this mode is
///   input-only, so the reports are accepted but not acted on.
///
/// The shared report is a [`Cell`](core::cell::Cell), not an atomic: the embassy
/// executor is single-threaded and cooperative, so the scanner and sender never touch
/// it at the same instant, and `[u8; REPORT_LEN]` is `Copy`.
///
/// # Leaving XInput
///
/// A host that has bound the Xbox 360 driver claims the kcp control interface along
/// with the rest of the device, so kcp can no longer switch the mode back. Holding
/// **Fn + Right Ctrl** ([`ESCAPE_KEY_FN`] + [`ESCAPE_KEY_RCTL`]) — the two adjacent
/// bottom-right keys — continuously for [`ESCAPE_HOLD`] calls
/// [`request_usb_mode`](crate::usb::request_usb_mode) with
/// [`UsbMode::Normal`](crate::usb::UsbMode::Normal) — the same path kcp `SET_USB_MODE`
/// takes, which signals the USB task to re-enumerate — then returns from the scanner,
/// ending this function. The hold is tracked off [`Instant`]: the start is stamped
/// when the combo first becomes fully held and cleared the moment either key lifts, so
/// only an unbroken hold fires.
///
/// While both keys are held the whole panel is overridden to a bright solid red — the
/// live RGB effect is saved first ([`EscapeHold`]) — so the combo is *visibly*
/// registering on hardware that silently swallowed the old escape. Releasing early
/// restores the saved effect; completing the hold leaves the override in place, as the
/// re-enumeration to Normal rebuilds RGB from the persisted config. The override is
/// painted through the shared [`crate::rgb`] state that the independent
/// [`rgb_task`](crate::rgb::rgb_task) renders every frame, so it shows even though this
/// loop drives no LEDs.
///
/// The caller additionally races this whole function against the mode-change signal,
/// so a kcp-driven switch (when the host has *not* claimed the control interface)
/// still cancels it cleanly.
pub async fn run(ep_in: &mut impl EndpointIn, ep_out: &mut impl EndpointOut) {
    // Latest input report, published by the scanner and read by the sender. A `Cell`
    // is correct here (no atomic needed): the executor is single-threaded, so the two
    // borrowers run cooperatively and never overlap, and the report is `Copy`.
    let report_cell = core::cell::Cell::new([0u8; REPORT_LEN]);

    // The only block that scans the matrix, so the escape combo is evaluated every
    // ~1 ms regardless of whether the host ever drains the IN endpoint.
    let scanner = async {
        let mut debouncer = matrix::Debouncer::new();
        // The in-progress escape hold (its start instant + the saved RGB effect it
        // overrides), or `None` when the combo is not fully held; taken the moment
        // either key lifts, so only an unbroken hold fires and an early release is
        // undone.
        let mut escape: Option<EscapeHold> = None;

        loop {
            // Scan the matrix once. The escape combo is evaluated on this *raw* scan
            // (a 1.5 s hold needs no debouncing, so the debouncer is not a variable in
            // whether the escape fires); the same scan is debounced below for the
            // gamepad report.
            let raw = matrix::scan();

            // Held-combo escape: the only way off XInput once the host's Xbox driver
            // has claimed the kcp control interface. While both keys are held the panel
            // is overridden to solid red as a visible signal (saving the prior effect
            // on the rising edge); holding for ESCAPE_HOLD requests Normal — signalling
            // the USB task to re-enumerate (the kcp SET_USB_MODE path) — and returns so
            // this function ends, while releasing early restores the saved effect.
            if escape_held(&raw) {
                let hold = escape.get_or_insert_with(EscapeHold::begin);
                if Instant::now() - hold.since >= ESCAPE_HOLD {
                    crate::usb::request_usb_mode(crate::usb::UsbMode::Normal as u8);
                    return;
                }
            } else if let Some(hold) = escape.take() {
                hold.restore();
            }

            report_cell.set(report(&debouncer.update(raw)));
            Timer::after(Duration::from_millis(1)).await;
        }
    };

    // Mirror the scanner's latest report onto the wire. A blocking write here cannot
    // starve the escape, since the scanner is an independent block.
    let sender = async {
        loop {
            ep_in.wait_enabled().await;
            // The host assumes the controller starts idle after every (re)enable, so the
            // dedup cache restarts empty and the first changed report is always sent.
            let mut last = [0u8; REPORT_LEN];

            loop {
                let rep = report_cell.get();
                if rep != last {
                    if ep_in.write(&rep).await.is_err() {
                        break; // host stopped draining (disconnect / mode left)
                    }
                    last = rep;
                }
                Timer::after(Duration::from_millis(1)).await;
            }
        }
    };

    // Accept the host's output reports (rumble / LED ring) and discard them: the
    // endpoint must be drained or the host's writes back up and NAK, but this mode is
    // input-only so nothing acts on them. A failed read just loops for the next one.
    let drain = async {
        let mut out_buf = [0u8; EP_MAX_PACKET as usize];
        loop {
            let _ = ep_out.read(&mut out_buf).await;
        }
    };

    // `select3`, not `join`: only the scanner returns (when the escape fires), and that
    // ends `run`. The sender and drain loops never return on their own — with no escape
    // all three stay pending and the caller's mode-change race ends the function then.
    select3(scanner, sender, drain).await;
}
