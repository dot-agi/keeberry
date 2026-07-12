// SPDX-License-Identifier: GPL-2.0-or-later
//! CH582 radio transport: the stop-and-wait TX queue, the RX/TX pump tasks, the
//! connection/pairing state machine, the report-routing helpers and the
//! kcp-over-radio bridge, sitting on top of the [`md`] codec and the
//! [`uart`](crate::uart) UART3 driver.
//!
//! It owns the whole radio path: link up, framed send/receive and ACKed
//! stop-and-wait delivery ([`tx_task`]/[`rx_task`]); the report-routing helpers
//! ([`send_keyboard`]/[`send_nkro`]/[`send_consumer`]) the keyboard loop calls;
//! device switch and pairing ([`devs_change`]); the auto-fallback transport
//! supervisor ([`transport_supervisor_task`]) that picks the best available link in
//! the priority order USB > 2.4 GHz > BT1/2/3; the power-on init burst and the
//! battery/charge cadence ([`housekeeping_task`]); and the 2.4G dongle's
//! kcp bridge ([`kcp_radio_task`]), which feeds inbound [`MdEvent`]s from
//! [`MD_EVENTS`] into the same [`kcp`] dispatcher that serves USB.
//!
//! # Stop-and-wait (smsg.c + module.c:237-291)
//!
//! Frames are delivered one at a time: send the head frame, wait up to
//! [`MD_SEND_PKT_TIMEOUT_MS`] for the radio's `61 0D 0A` sync ACK, retransmit on
//! timeout up to [`MD_SEND_PKT_RETRY`] times, then drop. Two vial-qmk fixes over
//! the upstream vendor source are reproduced here:
//!
//! * **The retry counter persists across retransmits** (`module.c:243-256`): a
//!   lost ACK must not reset the cap, or a wedged frame would retransmit forever
//!   and jam every later report. In [`tx_task`] the counter is a loop-local that
//!   is only re-initialised when the next frame is dequeued (the vendor resets
//!   `smsg_retry` only on `smsg_pop`).
//! * **Receive-before-transmit** (`module.c:286-289`): a just-arrived ACK must
//!   free the in-flight slot without an extra iteration's latency. Here the TX
//!   driver parks on [`ACK`] inside `with_timeout`, so the RX pump that produces
//!   the ACK is what wakes it; the next queued frame ships the instant the TX
//!   task is re-polled, with no polling-loop round-trip in between.

pub mod md;

use core::sync::atomic::{AtomicU8, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;
use embassy_time::{with_timeout, Duration, Instant, Timer};
use pac::Peripherals;

use crate::gpio::{self, Pin, Port};
use crate::kcp;
use crate::uart;
use md::{Frame, MdEvent, MdRx, MdState, MdStep};

// === Stop-and-wait parameters (smsg.c:14-19) ===
/// Per-frame ACK timeout in milliseconds (`MD_SNED_PKT_TIMEOUT`, smsg.c:13-15).
const MD_SEND_PKT_TIMEOUT_MS: u64 = 10;
/// Maximum retransmits before a frame is dropped (`MD_SEND_PKT_RETRY`,
/// smsg.c:17-19).
const MD_SEND_PKT_RETRY: u32 = 40;
/// Outbound frame queue depth. The vendor ring is `SMSG_NUM = 40` (smsg.c:7-9);
/// a smaller depth is used here to stay frugal with the 28 KB SRAM. Stop-and-wait
/// keeps only one frame in flight, so this only bounds how many reports may be
/// buffered behind it, which is ample.
const TX_QUEUE_DEPTH: usize = 16;
/// Decoded-event queue depth handed to [`kcp_radio_task`].
const EVENT_QUEUE_DEPTH: usize = 8;

/// The 3-byte sync ACK token echoed after every valid received frame
/// (`md_send_ack`, module.c:79-83). No checksum — it *is* the sync.
const MD_ACK: [u8; 3] = [0x61, 0x0D, 0x0A];

/// Outbound frames awaiting stop-and-wait delivery (the smsg ring).
static TX_QUEUE: Channel<CriticalSectionRawMutex, Frame, TX_QUEUE_DEPTH> = Channel::new();
/// Signalled by [`rx_task`] when the radio's sync ACK arrives, awaited by
/// [`tx_task`].
static ACK: Signal<CriticalSectionRawMutex, ()> = Signal::new();
/// Decoded inbound events for [`kcp_radio_task`] to consume.
pub static MD_EVENTS: Channel<CriticalSectionRawMutex, MdEvent, EVENT_QUEUE_DEPTH> = Channel::new();

/// Queue an 8-byte boot keyboard report, returning whether it was enqueued
/// (`false` when the queue is full — the `smsg_push` returning `false`
/// behaviour). The router uses this to decide whether to advance its "last sent"
/// cache.
pub fn md_send_kb(report: &[u8; 8]) -> bool {
    enqueue(Frame::keyboard(report))
}

/// Queue a 14-byte NKRO bitmap report (the overflow keys of the dual-send; see
/// [`send_nkro`]). Returns whether it was enqueued, like [`md_send_kb`].
pub fn md_send_nkro(report: &[u8; 14]) -> bool {
    enqueue(Frame::nkro(report))
}

/// Queue a 2-byte consumer-control usage, returning whether it was enqueued
/// (see [`md_send_kb`]).
pub fn md_send_consumer(usage: u16) -> bool {
    enqueue(Frame::consumer(usage))
}

/// Queue a 1-byte system-control bitmask, returning whether it was enqueued
/// (see [`md_send_kb`]).
pub fn md_send_system(bitmask: u8) -> bool {
    enqueue(Frame::system(bitmask))
}

/// Queue a 5-byte relative mouse report `[buttons, x, y, h, v]`, returning whether
/// it was enqueued (see [`md_send_kb`]).
pub fn md_send_mouse(buttons: u8, x: i8, y: i8, h: i8, v: i8) -> bool {
    enqueue(Frame::mouse(buttons, x, y, h, v))
}

/// Queue a 32-byte raw-HID frame.
pub fn md_send_raw(data: &[u8; md::MD_RAW_SIZE]) {
    enqueue(Frame::raw(data));
}

/// Queue a device-control command (one of the `md::DEVCTRL_*` sub-commands).
pub fn md_send_devctrl(sub: u8) {
    enqueue(Frame::devctrl(sub));
}

/// Queue a USB VID/PID advertisement.
pub fn md_send_vpid(vid: u16, pid: u16) {
    enqueue(Frame::vpid(vid, pid));
}

/// Queue a device-name advertisement; silently dropped if `name` is too long.
pub fn md_send_devinfo(name: &[u8]) {
    if let Some(frame) = Frame::devinfo(name) {
        enqueue(frame);
    }
}

/// Queue the dongle's USB manufacturer string (2.4G pairing); dropped if too long.
/// Pushed as UTF-16LE (see [`md::Frame::dongle_string`]).
pub fn md_send_manufacturer(name: &[u8]) {
    if let Some(frame) = Frame::manufacturer(name) {
        enqueue(frame);
    }
}

/// Queue the dongle's USB product string (2.4G pairing); dropped if too long.
/// Pushed as UTF-16LE (see [`md::Frame::dongle_string`]).
pub fn md_send_product(name: &[u8]) {
    if let Some(frame) = Frame::product(name) {
        enqueue(frame);
    }
}

/// Log a decoded inbound event over RTT for transport bring-up.
fn log_event(ev: &MdEvent) {
    match ev {
        MdEvent::RawOut(data) => {
            defmt::trace!("md rx: raw-HID 32B (first={=u8:#04x})", data[0])
        }
        MdEvent::Indicator(v) => defmt::trace!("md rx: indicator {=u8:#04x}", v),
        MdEvent::DevCtrl(st) => defmt::info!("md rx: state -> {}", st.name()),
        MdEvent::BatVol(v) => defmt::debug!("md rx: battery {=u8}%", v),
        MdEvent::FwVersion(v) => defmt::info!("md rx: radio fw v{=u8}", v),
        MdEvent::HostState(resume) => {
            defmt::debug!("md rx: host {}", if *resume { "resume" } else { "suspend" })
        }
    }
}

/// Push a built frame onto the stop-and-wait queue, returning whether it was
/// accepted (it is dropped, and `false` returned, when the queue is full).
fn enqueue(frame: Frame) -> bool {
    match TX_QUEUE.try_send(frame) {
        Ok(()) => true,
        Err(_) => {
            defmt::warn!("md: TX queue full, frame dropped");
            false
        }
    }
}

/// Bring up UART3 so the radio link is live.
///
/// The 100 ms power-on init burst (DEVCTRL/SLEEP/`devs_change` sequencing) runs
/// in [`housekeeping_task`]; this only brings the transport up.
pub fn init(p: &Peripherals) {
    uart::init(p);
}

// ===========================================================================
// Transport state (Devs; module.h:8-14, wireless.c:12)
// ===========================================================================

/// The active output transport (`DEVS_*`, module.h:8-14). [`Devs::Usb`] routes
/// HID reports through the USB writers; every other variant routes them over the
/// radio. The 5075B exposes USB, three BT profiles and the 2.4G dongle.
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Devs {
    /// Wired USB (`DEVS_USB`).
    Usb,
    /// BT profile 1 (`DEVS_BT1`).
    Bt1,
    /// BT profile 2 (`DEVS_BT2`).
    Bt2,
    /// BT profile 3 (`DEVS_BT3`).
    Bt3,
    /// 2.4G dongle (`DEVS_2G4`).
    G2_4,
}

impl Devs {
    /// The vendor `DEVS_*` numeric code (module.h:8-14): USB=0, BT1..3=1..3,
    /// 2G4=6. This is the byte the kcp WIRELESS group exchanges and the value
    /// stashed in [`TRANSPORT`].
    pub const fn code(self) -> u8 {
        match self {
            Devs::Usb => 0,
            Devs::Bt1 => 1,
            Devs::Bt2 => 2,
            Devs::Bt3 => 3,
            Devs::G2_4 => 6,
        }
    }

    /// Whether this transport is a wireless link (any variant but [`Devs::Usb`]).
    pub const fn is_wireless(self) -> bool {
        !matches!(self, Devs::Usb)
    }

    /// Parse a `DEVS_*` code, rejecting the BT4/BT5 codes this board lacks (and
    /// any other value) — the kcp WIRELESS `SET_MODE` bounds check.
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Devs::Usb),
            1 => Some(Devs::Bt1),
            2 => Some(Devs::Bt2),
            3 => Some(Devs::Bt3),
            6 => Some(Devs::G2_4),
            _ => None,
        }
    }

    /// The device name advertised for this transport — the identity the host on the
    /// matching link sees. The single source of truth for the "Akko 5075B <mode>"
    /// names: the wired USB product string ([`crate::usb`]), the 2.4G dongle's pushed
    /// product string and the three BT profiles' advertised names all resolve here,
    /// so every host is told which link it is on. All arms are `&'static` literals —
    /// no allocation (`no_std`). This replaces the vendor's stock per-profile naming
    /// (`PRODUCT " BTn"` and the dongle strings, module.c:25-51) with keeberry's own
    /// identity; `const fn` so the radio frame-length fits are asserted below at
    /// compile time.
    pub const fn device_name(self) -> &'static str {
        match self {
            Devs::Usb => "Akko 5075B USB",
            Devs::Bt1 => "Akko 5075B BT1",
            Devs::Bt2 => "Akko 5075B BT2",
            Devs::Bt3 => "Akko 5075B BT3",
            Devs::G2_4 => "Akko 5075B 2.4G",
        }
    }
}

/// The active transport — both QMK's `get_transport()` (USB vs wireless) and its
/// `wls_devs` (which wireless device), collapsed into one value. Default
/// [`Devs::Usb`] (wireless.c:12). A lock-free `u8` holding [`Devs::code`], like
/// the keymap's atomics.
static TRANSPORT: AtomicU8 = AtomicU8::new(0);

/// The current output transport.
pub fn transport() -> Devs {
    Devs::from_u8(TRANSPORT.load(Ordering::Relaxed)).unwrap_or(Devs::Usb)
}

/// Select the output transport (the `set_transport` + `wls_devs =` assignment of
/// `wireless_devs_change`). Prefer [`devs_change`] for a full switch that also
/// runs the pairing/connect sequence.
pub fn set_transport(d: Devs) {
    TRANSPORT.store(d.code(), Ordering::Relaxed);
}

// ===========================================================================
// Preferred wireless transport (the auto-fallback target)
// ===========================================================================

/// The user's preferred wireless transport — the fallback target when USB is
/// unplugged, and the head of the auto-fallback priority walk. Only ever a wireless
/// [`Devs`] (never [`Devs::Usb`]); an explicit kcp `SET_MODE`/`PAIR` to a wireless
/// mode sets it, and it is persisted in the CONFIG flash blob so a reboot resumes the
/// same channel ([`crate::config`]). Defaults to [`Devs::G2_4`] — the highest-priority
/// wireless — so a never-configured board still has a sensible fallback.
static PREFERRED_WLS: AtomicU8 = AtomicU8::new(Devs::G2_4.code());

/// The user's preferred wireless transport (the USB-unplug fallback target).
pub fn preferred_wireless() -> Devs {
    Devs::from_u8(PREFERRED_WLS.load(Ordering::Relaxed)).unwrap_or(Devs::G2_4)
}

/// Record the user's preferred wireless transport. [`Devs::Usb`] is ignored — USB is
/// the top priority, not a wireless fallback target, so selecting it never erases the
/// wireless the supervisor should fall back to.
pub fn set_preferred_wireless(d: Devs) {
    if d.is_wireless() {
        PREFERRED_WLS.store(d.code(), Ordering::Relaxed);
    }
}

/// Restore the preferred wireless transport from a persisted [`Devs`] code (the
/// CONFIG blob). An unknown or USB code leaves the [`Devs::G2_4`] default — fail-safe,
/// like the other restore paths.
pub fn set_preferred_wireless_code(code: u8) {
    if let Some(d) = Devs::from_u8(code) {
        set_preferred_wireless(d);
    }
}

/// Reset the preferred wireless transport to the [`Devs::G2_4`] power-on default (the
/// kcp `CONFIG.RESET` path).
pub fn reset_preferred_wireless() {
    set_preferred_wireless(Devs::G2_4);
}

/// The next wireless transport in the auto-fallback priority cycle
/// (2.4 GHz → BT1 → BT2 → BT3 → 2.4 GHz). The supervisor walks it when the active
/// wireless link stays down past [`WLS_CONNECT_TIMEOUT_MS`], so every paired channel
/// gets a turn. [`Devs::Usb`] is not in the cycle (it is the top priority, reached
/// only when the cable is present); it maps to the head so the match is total.
const fn next_wireless(d: Devs) -> Devs {
    match d {
        Devs::G2_4 => Devs::Bt1,
        Devs::Bt1 => Devs::Bt2,
        Devs::Bt2 => Devs::Bt3,
        Devs::Bt3 => Devs::G2_4,
        Devs::Usb => Devs::G2_4,
    }
}

// ===========================================================================
// Connection state machine (wireless.c:190-208, module.c:457-522)
// ===========================================================================

// The per-mode names ([`Devs::device_name`]) ride length-bounded radio frames when
// advertised: a BT profile name goes in a DEVINFO frame (≤ MD_SND_CMD_DEVINFO_LEN
// bytes) and the 2.4G dongle's product/manufacturer strings in dongle-string frames
// (≤ DONGLE_STRING_MAX bytes). The codec silently drops an over-long string, which
// would break pairing, so pin the fit at compile time — a future rename of a name or
// the brand cannot regress it unnoticed. (The names replace the vendor's stock
// per-profile naming, module.c:25-51, with keeberry's own Akko 5075B identity.)
const _: () = assert!(Devs::Bt1.device_name().len() <= md::MD_SND_CMD_DEVINFO_LEN);
const _: () = assert!(Devs::Bt2.device_name().len() <= md::MD_SND_CMD_DEVINFO_LEN);
const _: () = assert!(Devs::Bt3.device_name().len() <= md::MD_SND_CMD_DEVINFO_LEN);
// The 2.4G product and manufacturer strings are always pushed UTF-16LE (widened),
// doubling the on-wire byte count, so pin the *widened* length against the frame bound.
const _: () = assert!(2 * Devs::G2_4.device_name().len() <= md::DONGLE_STRING_MAX);
const _: () = assert!(2 * crate::usb::MANUFACTURER.len() <= md::DONGLE_STRING_MAX);
// The widening maps each byte `b` to the UTF-16LE unit `[b, 0]`, which only reproduces
// the character for ASCII (`< 0x80`); a non-ASCII byte would emit the wrong unit. The
// names are ASCII today — pin it so a future rename cannot silently reintroduce mojibake.
const _: () = assert!(is_ascii_str(Devs::G2_4.device_name()));
const _: () = assert!(is_ascii_str(crate::usb::MANUFACTURER));

/// Whether every byte of `s` is ASCII (`< 0x80`) — the precondition for the
/// [`md::Frame::dongle_string`] widening. Const so the asserts above run at build time.
const fn is_ascii_str(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() {
        if b[i] >= 0x80 {
            return false;
        }
        i += 1;
    }
    true
}

/// Switch the output transport and run its pairing/connect sequence.
///
/// Port of `wireless_devs_change(old, new, reset)` (wireless.c:190-208). Because
/// [`TRANSPORT`] encodes both USB-vs-wireless and which wireless device, storing
/// `new` performs the vendor's `set_transport()` and `wls_devs = new` at once.
/// On an actual device change or an explicit `reset`, the link state is dropped
/// to [`MdState::Disconnected`] and the indicator cleared (wireless.c:197-200)
/// so a stale "connected" cannot mask the reconnect.
pub fn devs_change(new: Devs, reset: bool) {
    let old = transport();
    if old != new || reset {
        md::set_state(MdState::Disconnected);
        md::set_indicator(0);
    }
    set_transport(new);
    md_devs_change(new, reset);
}

/// The per-target select + pairing sequence (`md_devs_change`, module.c:457-522).
///
/// `reset` requests a fresh pairing (clear the bond, re-advertise, pair);
/// without it the call only re-selects the channel to reconnect to the existing
/// bond. The 2.4G branch adapts the vendor's wide-string manufacturer/product push
/// (module.c:463-472): the dongle needs the strings as UTF-16LE or the host renders
/// them as CJK mojibake, so keeberry always widens them (see the note at the push).
fn md_devs_change(devs: Devs, reset: bool) {
    match devs {
        Devs::Usb => md_send_devctrl(md::DEVCTRL_USB),
        Devs::G2_4 => {
            md_send_devctrl(md::DEVCTRL_2G4);
            if reset {
                // Advertise the keyboard's USB identity to the dongle, clear the
                // bond, pair (module.c:466-477). The product string is the
                // "Akko 5075B 2.4G" mode name, so the dongle's host sees the 2.4G
                // link named for what it is; VID/PID stay the shared board identity.
                // The dongle copies these strings verbatim into UTF-16LE USB string
                // descriptors, so a plain-ASCII push shows up on the host as one CJK
                // code unit per byte-pair (mojibake); they are always pushed
                // pre-widened to UTF-16LE. The vendor version-gates the widening on
                // md_get_version() >= 48, but that reports the *keyboard radio's*
                // firmware, not the dongle's: this board reports radio v1 (confirmed
                // over kcp) yet its dongle still mojibaked the ASCII form, so the
                // radio version says nothing about the dongle's descriptor encoding.
                // keeberry targets this UTF-16 dongle, so it always widens.
                md_send_manufacturer(crate::usb::MANUFACTURER.as_bytes());
                md_send_product(devs.device_name().as_bytes());
                md_send_vpid(crate::usb::VID, crate::usb::PID);
                md_send_devctrl(md::DEVCTRL_CLEAN);
                md_send_devctrl(md::DEVCTRL_PAIR);
            }
        }
        Devs::Bt1 => bt_select(md::DEVCTRL_BT1, Devs::Bt1.device_name(), reset),
        Devs::Bt2 => bt_select(md::DEVCTRL_BT2, Devs::Bt2.device_name(), reset),
        Devs::Bt3 => bt_select(md::DEVCTRL_BT3, Devs::Bt3.device_name(), reset),
    }
}

/// BTn select, plus (on `reset`) clear-bond → advertise-name → pair
/// (module.c:479-518; identical across BT1..BT5 bar the devctrl and name). `name`
/// is the profile's "Akko 5075B BTn" [`Devs::device_name`], advertised so the
/// profile appears under that name in the host's Bluetooth list.
fn bt_select(devctrl: u8, name: &str, reset: bool) {
    md_send_devctrl(devctrl);
    if reset {
        md_send_devctrl(md::DEVCTRL_CLEAN);
        md_send_devinfo(name.as_bytes());
        md_send_devctrl(md::DEVCTRL_PAIR);
    }
}

// ===========================================================================
// Report routing helpers (wireless.c:44-182)
// ===========================================================================

/// Send an 8-byte boot keyboard report over the radio, gated on the link being
/// up (`wireless_send_keyboard`, wireless.c:44-56).
///
/// While disconnected the report is dropped and a connect attempt is re-issued
/// for the current device — exactly the vendor behaviour. The caller
/// ([`keyboard_loop`](crate::usb)) only invokes this on a report *change*, so a
/// held key triggers a single reconnect rather than one per scan. Returns
/// whether the report was actually enqueued: `false` if the link was down (a
/// reconnect was issued instead) **or** the TX queue was full, so the caller
/// leaves its "last sent" cache untouched and retries on the next scan — the
/// report is never recorded as sent when it was not.
pub fn send_keyboard(report: &[u8; 8]) -> bool {
    if md::state() != MdState::Connected {
        devs_change(transport(), false);
        return false;
    }
    md_send_kb(report)
}

/// Dual-send a split NKRO report over the radio, gated on the link
/// (`wireless_send_nkro`, wireless.c:58-130).
///
/// The vendor sends an 8-byte boot report carrying the keys that fit the six
/// rollover slots *plus* a 14-byte bitmap of the keys that overflow them, so the
/// two never share a usage and the dongle reconstructs full N-key rollover without
/// a USB protocol flag. keeberry computes that split in the keymap engine — the
/// lowest six held usages are already in `boot`, the rest in `overflow` — so this
/// only frames the two halves the vendor does. Mirrors [`send_keyboard`]: while
/// disconnected both halves are dropped and a reconnect re-issued, and the return
/// is whether the report was fully enqueued (both halves), so the caller leaves
/// its "last sent" cache untouched and retries — the resend is idempotent (both
/// halves are level state), so a retried frame cannot double-type.
pub fn send_nkro(boot: &[u8; 8], overflow: &[u8; 14]) -> bool {
    if md::state() != MdState::Connected {
        devs_change(transport(), false);
        return false;
    }
    // Enqueue both halves; the caller advances its cache only when both made it
    // onto the TX queue (a dropped half is retried next scan, never recorded sent).
    let boot_ok = md_send_kb(boot);
    let overflow_ok = md_send_nkro(overflow);
    boot_ok && overflow_ok
}

/// Send one consumer-control / system usage over the radio, gated on the link
/// (`wireless_send_extra`, wireless.c:159-182).
///
/// Usages `0x81..=0x83` are the System Control page (power/sleep/wake) and go out
/// as a system bitmask `1 << (usage - 0x81)`; everything else is a consumer-page
/// usage. While disconnected the usage is dropped and a connect attempt
/// re-issued, like [`send_keyboard`]. Returns whether it was enqueued.
pub fn send_consumer(usage: u16) -> bool {
    if md::state() != MdState::Connected {
        devs_change(transport(), false);
        return false;
    }
    // The tail value is the enqueue result, so the caller only advances its
    // "last sent" cache when the usage actually went onto the TX queue.
    match usage {
        0x81..=0x83 => md_send_system(1u8 << (usage - 0x81)),
        _ => md_send_consumer(usage),
    }
}

/// Send one relative mouse report over the radio, gated on the link
/// (`wireless_send_mouse`, wireless.c:132-157).
///
/// The 5-byte vendor frame is `[buttons, x, y, h, v]` — the horizontal wheel then
/// the vertical wheel; keeberry's mouse keys scroll only vertically, so the caller
/// passes `h = 0` and `v` carries the wheel. While disconnected the report is
/// dropped and a connect attempt re-issued, like [`send_keyboard`]. Returns whether
/// it was enqueued, so the caller advances its "last sent" cache only on a real send.
pub fn send_mouse(buttons: u8, x: i8, y: i8, h: i8, v: i8) -> bool {
    if md::state() != MdState::Connected {
        devs_change(transport(), false);
        return false;
    }
    md_send_mouse(buttons, x, y, h, v)
}

// ===========================================================================
// kcp WIRELESS group support (read state + control, called from kcp::handle)
// ===========================================================================

/// Latest radio-reported battery level 0..=100 (`md_info.bat`).
pub fn battery() -> u8 {
    md::battery()
}

/// Current connection state as its `MD_STATE_*` code (module.h:17-23).
pub fn connection_state() -> u8 {
    md::state().as_u8()
}

/// Radio firmware version (`md_info.version`).
pub fn radio_version() -> u8 {
    md::version()
}

/// Clear the active channel's bond (`DEVCTRL_CLEAN`) — kcp WIRELESS `UNPAIR`.
pub fn unpair() {
    md_send_devctrl(md::DEVCTRL_CLEAN);
}

/// Enable/disable the radio's 30-minute idle sleep per mode (`SLEEP_*_EN/DIS`),
/// the kcp WIRELESS `SET_SLEEP_POLICY` op (and part of the power-on burst).
pub fn set_sleep_policy(enable_bt: bool, enable_2g4: bool) {
    md_send_devctrl(if enable_bt {
        md::DEVCTRL_SLEEP_BT_EN
    } else {
        md::DEVCTRL_SLEEP_BT_DIS
    });
    md_send_devctrl(if enable_2g4 {
        md::DEVCTRL_SLEEP_2G4_EN
    } else {
        md::DEVCTRL_SLEEP_2G4_DIS
    });
}

/// Ask the radio to report its battery level (`md_inquire_bat`, module.c:524-533).
/// The vendor gates on `!smsg_is_busy()`; here [`enqueue`] already drops the
/// request when the TX queue is full, so it is simply enqueued.
pub fn inquire_battery() {
    md_send_devctrl(md::DEVCTRL_INQVOL);
}

// ===========================================================================
// kcp over the radio — the validated 2.4G dongle bridge
// ===========================================================================

/// Bridge inbound radio events into the rest of the firmware.
///
/// Drains [`MD_EVENTS`] (filled by [`rx_task`]). A [`MdEvent::RawOut`] is a
/// host raw-HID report the 2.4G dongle forwarded verbatim on usage page
/// `0xFF60`; it is dispatched through the *same* [`kcp::handle`] that serves USB
/// and the reply is framed straight back over the radio (`md_receive_raw_cb` +
/// `md_send_raw`, module.c:187-198, 442-455) — the validated dongle bridge, so
/// the full kcp config surface works over 2.4G. [`MdEvent::HostState`] is the
/// host suspend/resume notification. The persistent-state events
/// (indicator / devctrl / battery / fw-version) were already folded into
/// [`MdInfo`](md) by [`md::apply_event`] in [`rx_task`], so they need no routing.
#[embassy_executor::task]
pub async fn kcp_radio_task() {
    loop {
        match MD_EVENTS.receive().await {
            MdEvent::RawOut(buf) => {
                let reply = kcp::handle(&buf);
                md_send_raw(&reply);
            }
            MdEvent::HostState(resume) => {
                // md_receive_host_cb (module.c:226-228). keeberry does not implement
                // host-driven power management; the transition is logged for
                // diagnostics only.
                defmt::debug!("wls: host {}", if resume { "resume" } else { "suspend" });
            }
            _ => {}
        }
    }
}

// ===========================================================================
// Power-on init burst + battery / charge cadence
// ===========================================================================

/// Power-on settle before the init burst (`post_init_timer` window, ansi.c:374).
const POWER_ON_DELAY_MS: u64 = 100;
/// Battery inquiry period (`WLS_INQUIRY_BAT_TIME`, wireless.c:8-10).
const INQUIRY_BAT_TIME_MS: u64 = 3000;

/// USB-cable insertion sense (`HS_BAT_CABLE_PIN = A7`, config.h:8). Floating
/// input, externally driven high while the cable is inserted (ansi.c:220-222).
const BAT_CABLE_PIN: Pin = Pin::new(Port::A, 7);
/// Battery-full sense (`BAT_FULL_PIN = A15`, config.h:11). Pulled-up input
/// (ansi.c:224-226).
const BAT_FULL_PIN: Pin = Pin::new(Port::A, 15);

/// Push the charge state to the radio (`housekeeping_task_user`, ansi.c:1276-1291):
/// cable-in **and** battery-full ⇒ `CHARGING_DONE`, cable-in ⇒ `CHARGING`, else
/// `CHARGING_STOP`.
///
// Both pins are read at their raw level, matching the vendor's `gpio_read_pin`.
// `BAT_FULL_PIN` is brought up with a pull-up, so the board's "full" signal may be
// active-low — its polarity is unconfirmed on hardware. Charge state is advisory
// only (it affects neither the link nor keystrokes), so the raw-faithful read ships
// as-is and `CHARGING_DONE` is treated as an advisory hint, not a guarantee.
fn push_charge_state() {
    let charging = gpio::is_high(BAT_CABLE_PIN);
    let full = gpio::is_high(BAT_FULL_PIN);
    let sub = if charging && full {
        md::DEVCTRL_CHARGING_DONE
    } else if charging {
        md::DEVCTRL_CHARGING
    } else {
        md::DEVCTRL_CHARGING_STOP
    };
    md_send_devctrl(sub);
}

/// The wireless housekeeping task: the 100 ms power-on init burst, then the
/// periodic battery / charge cadence.
///
/// The burst (after the settle delay) mirrors `wireless_post_task`
/// (ansi.c:371-385): request the radio firmware version, enable the BT and 2.4G
/// idle-sleep timers, then select the boot transport (no reset → reconnect to
/// the stored bond). The periodic loop mirrors the battery inquiry of
/// `wireless_task` (wireless.c:230-240) and the charge push of
/// `housekeeping_task_user` (ansi.c:1276-1293), both gated to wireless mode:
/// on USB the host *is* the link, and (with no radio present) emitting to the
/// UART would only burn stop-and-wait retries.
#[embassy_executor::task]
pub async fn housekeeping_task() {
    // Configure the charge-sense inputs once. GPIOA is already clocked (the
    // matrix and USB bring-up both enable it), so this needs no Peripherals.
    gpio::set_input_floating(BAT_CABLE_PIN);
    gpio::set_input_pull_up(BAT_FULL_PIN);

    Timer::after(Duration::from_millis(POWER_ON_DELAY_MS)).await;

    // One-shot init burst (ansi.c:376-379).
    md_send_devctrl(md::DEVCTRL_FW_VERSION);
    set_sleep_policy(true, true);
    // Select the boot transport on the radio without a pairing reset, so it
    // reconnects to the last bond (ansi.c:379, wireless_devs_change(.., false)).
    devs_change(transport(), false);

    loop {
        Timer::after(Duration::from_millis(INQUIRY_BAT_TIME_MS)).await;
        if transport() != Devs::Usb {
            push_charge_state();
            inquire_battery();
        }
    }
}

// ===========================================================================
// Auto-fallback transport supervisor (priority: USB > 2.4 GHz > BT1/2/3)
// ===========================================================================
//
// The report routers ([`send_keyboard`] et al.) only ever *reconnect within* the
// current transport; they never change it. This supervisor is the piece that selects
// the best AVAILABLE transport and switches between them, in the user's priority
// order:
//
//   1. USB — whenever a real wired host is present (cable in *and* the host has
//      enumerated us). The top priority: on a USB plug the supervisor takes it,
//      tearing down any wireless session.
//   2. The preferred wireless ([`preferred_wireless`], default 2.4 GHz) — the
//      fallback when USB is absent.
//   3. The remaining wireless channels, walked in priority order ([`next_wireless`])
//      when the preferred one will not link, so a paired BT profile is found when
//      2.4 GHz is not paired.
//
// # Detecting USB presence
//
// The MUSB core does not report VBUS removal (its embassy driver leaves it a TODO),
// so [`crate::usb::usb_configured`] alone cannot see an unplug — it stays set. The
// authoritative plug/unplug edge is therefore the hardware cable-sense pin
// ([`BAT_CABLE_PIN`], VBUS-derived). USB counts as present only when the cable is in
// *and* a host has configured us, so a charge-only cable (VBUS, no enumeration) is
// not mistaken for a wired host and cannot steal a working wireless session.
//
// # Debounce (don't thrash)
//
// A brief cable wobble or a host bus-reset glitch must not tear down a wireless (or
// wired) session. The combined signal is debounced with an asymmetric window: quick
// to *adopt* USB (it is preferred, and the enumeration requirement already filters
// out VBUS glitches) but slower to *release* it, which also rides out a slow host
// enumeration at boot without bouncing the transport.
//
// # Not fighting the user / pairing
//
// USB is taken only on the plug/unplug *edge*, never as a continuous clamp, so an
// explicit kcp `SET_MODE` to a wireless channel while the cable is in stands until
// the next USB edge. And the channel walk pauses while the radio reports `PAIRING`,
// so a user-initiated pair (which can take far longer than the connect timeout) is
// never interrupted.

/// Supervisor evaluation period.
const SUPERVISOR_TICK_MS: u64 = 100;
/// Debounce for *adopting* USB: the combined USB-present signal must hold this long
/// before switching to USB. Short, because the signal already requires host
/// enumeration ([`crate::usb::usb_configured`]) — a mere VBUS glitch never satisfies
/// it — so this only paces a genuine plug-in.
const USB_ADOPT_DEBOUNCE_MS: u64 = 300;
/// Debounce for *releasing* USB back to wireless: the USB-present signal must be gone
/// this long before tearing down the wired session. Deliberately longer than
/// adoption, so a brief cable wobble or a host bus-reset does not drop the user onto a
/// slower link, and a slow host enumeration at boot does not bounce the transport.
const USB_RELEASE_DEBOUNCE_MS: u64 = 600;
/// How long the supervisor waits for the active wireless candidate to reach
/// `CONNECTED` before advancing to the next channel in the priority cycle. Generous
/// enough for a real bond to reconnect (sub-second to a couple of seconds), short
/// enough that hunting for a paired channel is not sluggish. Paused while `PAIRING`.
const WLS_CONNECT_TIMEOUT_MS: u64 = 5000;
/// Wireless channels in the fallback cycle (2.4G + BT1/2/3): one full pass of advances
/// before the supervisor rests on the preferred channel (reconnecting to its bond)
/// rather than cycling forever on a board with nothing paired.
const WLS_CHANNELS: u8 = 4;

/// The auto-fallback transport supervisor: selects the best available transport in
/// the priority order USB > 2.4 GHz > BT1/2/3 and switches between them.
///
/// See the section header above for the policy. Owns transport *selection* (the
/// report routers own only per-transport reconnect); a dedicated task so its 100 ms
/// cadence is independent of the 3 s battery cadence in [`housekeeping_task`], which
/// keeps running the vendor boot burst.
#[embassy_executor::task]
pub async fn transport_supervisor_task() {
    // The cable-sense pin is the authoritative "USB cable present" signal. Configure
    // it here so the supervisor does not depend on `housekeeping_task`'s ordering;
    // both set the same floating-input mode (the external VBUS divider drives it).
    gpio::set_input_floating(BAT_CABLE_PIN);

    // Debounced USB-present state. Seeded `true` to match the boot transport
    // (`TRANSPORT` defaults to USB), so a battery boot sees the first stable no-USB
    // reading as a falling edge and falls back to the preferred wireless.
    let mut usb_present = true;
    // Consecutive-time counters for the asymmetric debounce.
    let mut adopt_ms: u64 = 0;
    let mut release_ms: u64 = 0;

    // Wireless fallback bookkeeping (meaningful only while off USB). `last_seen` tracks
    // the transport so an external change (an explicit `SET_MODE`) restarts the connect
    // clock, giving the newly chosen channel a full window before any walk.
    let mut last_seen = transport();
    let mut since_switch = Instant::now();
    let mut advances_left = WLS_CHANNELS;

    loop {
        Timer::after(Duration::from_millis(SUPERVISOR_TICK_MS)).await;

        // --- Debounce USB presence ---
        // Adopt USB only when a real data host is present (cable AND host-configured);
        // release it only on a genuine loss of that host. A cable that is in but
        // unconfigured is *held*, not released, while a USB personality is
        // re-enumerating (a mode switch drops CONFIGURED for up to MODE_CONFIRM with
        // the cable still in) — otherwise it releases, so a charge-only source (VBUS,
        // no host) falls to wireless and you can still type while charging.
        let cable = gpio::is_high(BAT_CABLE_PIN);
        if !cable {
            // No cable → no host. Clear a CONFIGURED left stale-true by an unplug (the
            // MUSB core reports no VBUS-off), so a later charge-only reinsert is not
            // read as a wired host.
            crate::usb::clear_configured();
        }
        let host_present = cable && crate::usb::usb_configured();

        let edge = if usb_present {
            // On USB: release when the host is gone for good — but hold through a
            // personality re-enumeration (cable in, transiently unconfigured).
            if host_present || crate::usb::usb_personality_active() {
                release_ms = 0;
                false
            } else {
                release_ms += SUPERVISOR_TICK_MS;
                release_ms >= USB_RELEASE_DEBOUNCE_MS
            }
        } else {
            // Off USB: adopt once a real host has been present long enough.
            if host_present {
                adopt_ms += SUPERVISOR_TICK_MS;
                adopt_ms >= USB_ADOPT_DEBOUNCE_MS
            } else {
                adopt_ms = 0;
                false
            }
        };

        if edge {
            usb_present = !usb_present;
            adopt_ms = 0;
            release_ms = 0;
            if usb_present {
                // USB arrived — the top priority. Switch to it, dropping any wireless.
                if transport() != Devs::Usb {
                    devs_change(Devs::Usb, false);
                }
            } else {
                // USB left — fall back to the preferred wireless. The `t != last_seen`
                // check below picks up the transport change and arms the fresh fallback
                // cycle, so that bookkeeping is not duplicated here.
                devs_change(preferred_wireless(), false);
            }
        }

        // An external transport change (an explicit `SET_MODE`, or the edge switch just
        // above) restarts the fallback clock so the new channel gets a full connect
        // window before the supervisor walks on.
        let t = transport();
        if t != last_seen {
            last_seen = t;
            since_switch = Instant::now();
            advances_left = WLS_CHANNELS;
        }

        // --- Wireless fallback management (only while genuinely off USB) ---
        // Left alone when USB is present, including a user who explicitly picked a
        // wireless mode while the cable is in — an override that stands until the next
        // USB edge.
        if !usb_present && t.is_wireless() {
            match md::state() {
                // Linked: hold, and refill the budget so a future drop gets a full cycle.
                MdState::Connected => {
                    advances_left = WLS_CHANNELS;
                    since_switch = Instant::now();
                }
                // Pairing in progress (a user-initiated pair): hold the clock so the
                // walk never interrupts it.
                MdState::Pairing => since_switch = Instant::now(),
                // Down: after the connect timeout, walk to the next channel to find a
                // paired one; once a full pass finds nothing, rest on the preferred
                // channel and keep trying to reconnect to its bond (a later pair or USB
                // edge revives the cross-channel walk).
                _ => {
                    if since_switch.elapsed() >= Duration::from_millis(WLS_CONNECT_TIMEOUT_MS)
                        && advances_left > 0
                    {
                        let next = next_wireless(t);
                        advances_left -= 1;
                        since_switch = Instant::now();
                        last_seen = next;
                        devs_change(next, false);
                    }
                }
            }
        }
    }
}

/// RX pump: UART3 → [`MdRx`] state machine → ACK echo / [`MdInfo`](md) update /
/// [`MD_EVENTS`].
///
/// Mirrors `md_receive_msg_task` (module.c:121-235): every checksum-valid frame
/// is acknowledged with the 3-byte token and decoded; the bare sync ACK frees
/// the stop-and-wait slot without echoing anything back.
#[embassy_executor::task]
pub async fn rx_task() {
    let mut rx = MdRx::new();
    loop {
        let byte = uart::read().await;
        match rx.push(byte) {
            MdStep::Pending => {}
            MdStep::AckToken => {
                // The radio acknowledged our in-flight frame.
                ACK.signal(());
            }
            MdStep::Frame(event) => {
                // Echo the sync ACK for every valid received frame.
                uart::write_all(&MD_ACK).await;
                if let Some(event) = event {
                    // Observe the decoded event over RTT (transport bring-up
                    // visibility), fold persistent state into MdInfo, then hand it
                    // to `kcp_radio_task` via MD_EVENTS. A full queue drops the
                    // event — MdInfo has already captured any persistent state.
                    log_event(&event);
                    md::apply_event(&event);
                    let _ = MD_EVENTS.try_send(event);
                }
            }
        }
    }
}

/// TX pump: the stop-and-wait driver (`md_send_pkt_task`, module.c:237-274).
///
/// Dequeues one frame, transmits it, and waits for the radio's sync ACK,
/// retransmitting on timeout with the retry counter persisting across attempts.
#[embassy_executor::task]
pub async fn tx_task() {
    loop {
        // smsg_peek + smsg_pop: take the head frame (blocks until one is
        // queued). Holding it locally lets us retransmit without re-reading the
        // queue.
        let frame = TX_QUEUE.receive().await;

        // smsg_retry: re-initialised here, i.e. only when a new head frame is
        // dequeued — never between this frame's own retransmits.
        let mut retry: u32 = 0;
        loop {
            // Only ACKs that arrive after this transmission count.
            ACK.reset();
            uart::write_all(frame.as_bytes()).await;

            match with_timeout(Duration::from_millis(MD_SEND_PKT_TIMEOUT_MS), ACK.wait()).await {
                // smsg_state_replied: ACKed — move on to the next frame.
                Ok(()) => break,
                // Timed out (smsg_state_retry): retransmit until the cap, then
                // drop. The counter persists across attempts.
                Err(_) => {
                    retry += 1;
                    if retry > MD_SEND_PKT_RETRY {
                        defmt::warn!("md: frame dropped after {} retries", MD_SEND_PKT_RETRY);
                        break;
                    }
                }
            }
        }
    }
}
