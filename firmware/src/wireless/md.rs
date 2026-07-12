// SPDX-License-Identifier: GPL-2.0-or-later
//! The `md` (module) wire protocol spoken to the CH582F radio over UART3.
//!
//! Faithful port of the vendor QMK module codec
//! (`vial-qmk keyboards/linker/wireless/module.c` + `module.h`). vial-qmk's
//! `module.c` carries two fixes over the upstream vendor source — the
//! persist-across-retransmits retry counter and the receive-before-transmit
//! ordering — both of which live in the stop-and-wait driver
//! ([`super`](crate::wireless)); this file is the pure codec they sit on.
//!
//! # Framing
//!
//! Every frame, both directions, is `[opcode][payload…][checksum]`, where the
//! checksum is the 8-bit additive sum of all preceding bytes
//! (`module.c:85-103`). Multi-byte fields are little-endian. The payload length
//! is implicit per opcode (the `MD_SND_CMD_*_LEN` table, `module.h:25-35`)
//! except where a length byte is carried inline: the raw-HID frames put it at
//! index 2 and `DEVINFO` at index 1.
//!
//! The 3-byte token `61 0D 0A` is the **sync ACK** — it has no checksum and is
//! both echoed by us after every valid received frame and sent by the radio
//! after every frame we transmit (the stop-and-wait acknowledgement,
//! `module.c:79-83, 153-160`).

use core::sync::atomic::{AtomicU8, Ordering};

// ===========================================================================
// Opcodes and payload lengths (module.h)
// ===========================================================================

// --- TX (keyboard → radio) report opcodes (module.h:37-51) ---
/// 8-byte boot keyboard report.
const MD_SND_CMD_SEND_KB: u8 = 0xA1;
/// 14-byte NKRO bitmap report.
const MD_SND_CMD_SEND_NKRO: u8 = 0xA2;
/// 2-byte consumer-control usage (little-endian).
const MD_SND_CMD_SEND_CONSUMER: u8 = 0xA3;
/// 1-byte system-control bitmask.
const MD_SND_CMD_SEND_SYSTEM: u8 = 0xA4;
/// 5-byte relative mouse report `[buttons, x, y, h, v]` (module.h:44).
const MD_SND_CMD_SEND_MOUSE: u8 = 0xA8;
/// `[len][name]` device-name advertisement.
const MD_SND_CMD_SEND_DEVINFO: u8 = 0xA9;
/// `[len][str]` dongle USB manufacturer-string advertisement (module.h:47).
const MD_SND_CMD_MANUFACTURER: u8 = 0xAB;
/// `[len][str]` dongle USB product-string advertisement (module.h:48).
const MD_SND_CMD_PRODUCT: u8 = 0xAC;
/// 4-byte `(pid << 16) | vid` USB VID/PID.
const MD_SND_CMD_VPID: u8 = 0xAD;
/// Raw-HID frame `[0x61][32][32 bytes]`.
const MD_SND_CMD_RAW: u8 = 0xAF;
/// Raw-HID inbound sub-opcode (the second byte of a transmitted raw frame).
const MD_SND_CMD_RAW_IN: u8 = 0x61;
/// Device-control frame `[sub]`.
const MD_SND_CMD_DEVCTRL: u8 = 0xA6;

// --- TX payload lengths (module.h:25-35) ---
const MD_SND_CMD_KB_LEN: usize = 8;
const MD_SND_CMD_NKRO_LEN: usize = 14;
const MD_SND_CMD_CONSUMER_LEN: usize = 2;
const MD_SND_CMD_SYSTEM_LEN: usize = 1;
const MD_SND_CMD_MOUSE_LEN: usize = 5;
/// Maximum device-name length carried by a `DEVINFO` frame. `pub(crate)` so the
/// connection state machine can assert its advertised BT names fit at compile time.
pub(crate) const MD_SND_CMD_DEVINFO_LEN: usize = 18;

// --- DEVCTRL sub-commands (module.h:54-76), driven by the connection state machine ---
/// Switch the radio to USB/wired output.
pub const DEVCTRL_USB: u8 = 0x11;
/// Switch the radio to the 2.4G dongle channel.
pub const DEVCTRL_2G4: u8 = 0x30;
/// Switch to BT profile 1..=3 (the board's three BT slots).
pub const DEVCTRL_BT1: u8 = 0x31;
pub const DEVCTRL_BT2: u8 = 0x32;
pub const DEVCTRL_BT3: u8 = 0x33;
/// Begin pairing on the active channel.
pub const DEVCTRL_PAIR: u8 = 0x51;
/// Clear the active channel's bond.
pub const DEVCTRL_CLEAN: u8 = 0x52;
/// Ask the radio to report its battery level.
pub const DEVCTRL_INQVOL: u8 = 0x53;
/// Enable / disable the 30-minute idle sleep, per radio mode.
pub const DEVCTRL_SLEEP_BT_EN: u8 = 0x55;
pub const DEVCTRL_SLEEP_BT_DIS: u8 = 0x56;
pub const DEVCTRL_SLEEP_2G4_EN: u8 = 0x57;
pub const DEVCTRL_SLEEP_2G4_DIS: u8 = 0x58;
/// Battery charge state notifications.
pub const DEVCTRL_CHARGING: u8 = 0x64;
pub const DEVCTRL_CHARGING_STOP: u8 = 0x65;
pub const DEVCTRL_CHARGING_DONE: u8 = 0x66;
/// Request the radio firmware version.
pub const DEVCTRL_FW_VERSION: u8 = 0x70;

// --- RX (radio → keyboard) opcodes (module.h:80-101) ---
/// Raw-HID frame from the host: `[0xAF][0x60][32][32 bytes]`.
const MD_REV_CMD_RAW: u8 = 0xAF;
/// Raw-HID outbound sub-opcode (second byte of a received raw frame).
const MD_REV_CMD_RAW_OUT: u8 = 0x60;
/// Host keyboard-LED indicator state, 1 byte.
const MD_REV_CMD_INDICATOR: u8 = 0x5A;
/// Connection-state change `[sub]`.
const MD_REV_CMD_DEVCTRL: u8 = 0x5B;
/// Battery level 0..=100, 1 byte.
const MD_REV_CMD_BATVOL: u8 = 0x5C;
/// Radio firmware version, 1 byte.
const MD_REV_CMD_MD_FW_VERSION: u8 = 0x5D;
/// Host suspend/resume notification, 1 byte (also the raw sub-opcode value, but
/// only ever a *first* byte here — disambiguated by position).
const MD_REV_CMD_HOST_STATE: u8 = 0x60;

// --- RX DEVCTRL sub-commands (module.h:88-93) ---
const MD_REV_CMD_DEVCTRL_PAIRING: u8 = 0x31;
const MD_REV_CMD_DEVCTRL_CONNECTED: u8 = 0x32;
const MD_REV_CMD_DEVCTRL_DISCONNECTED: u8 = 0x33;
const MD_REV_CMD_DEVCTRL_REJECT: u8 = 0x36;

// --- HOST_STATE payload values (module.h:100-101) ---
const MD_REV_CMD_HOST_STATE_RESUME: u8 = 0x01;

/// Raw-HID payload size (`MD_RAW_SIZE`, smsg.c:54).
pub const MD_RAW_SIZE: usize = 32;

/// Maximum framed length, `MD_RAW_SIZE + 4` — the raw-HID frame
/// `[0xAF][sub][32][32 bytes][checksum]` (`MD_SEND_PKT_PAYLOAD_MAX`,
/// smsg.c:21-23). Every [`Frame`] this codec builds fits within it.
pub const FRAME_MAX: usize = MD_RAW_SIZE + 4;

/// Maximum dongle manufacturer/product string length that fits a [`Frame`]:
/// `FRAME_MAX` minus the opcode, length byte and trailing checksum. This bounds the
/// **on-wire** byte count, which for a wide (UTF-16LE) string
/// is twice the character count. See [`Frame::dongle_string`]; the advertised strings
/// fit widened (the longest, "Akko 5075B 2.4G", is 30 of the 33 bytes), and a longer
/// one is rejected rather than overflowing the buffer. `pub(crate)` so the connection
/// state machine can assert its 2.4G strings fit at compile time.
pub(crate) const DONGLE_STRING_MAX: usize = FRAME_MAX - 3;

// ===========================================================================
// Connection state (md_info_t.state; MD_STATE_* in module.h:17-23)
// ===========================================================================

/// Radio connection state, mirrored from `DEVCTRL` notifications.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MdState {
    /// `MD_STATE_NONE` — no connection activity observed.
    None,
    /// `MD_STATE_PAIRING` — advertising / pairing.
    Pairing,
    /// `MD_STATE_CONNECTED` — link established.
    Connected,
    /// `MD_STATE_DISCONNECTED` — link dropped.
    Disconnected,
    /// `MD_STATE_REJECT` — host rejected the connection.
    Reject,
}

impl MdState {
    /// The vendor `MD_STATE_*` numeric code (module.h:17-23), used both for the
    /// [`MD_STATE`] atomic and the kcp WIRELESS `GET_STATE` reply.
    pub const fn as_u8(self) -> u8 {
        match self {
            MdState::None => 0,
            MdState::Pairing => 1,
            MdState::Connected => 2,
            MdState::Disconnected => 3,
            MdState::Reject => 4,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => MdState::Pairing,
            2 => MdState::Connected,
            3 => MdState::Disconnected,
            4 => MdState::Reject,
            _ => MdState::None,
        }
    }

    /// A short label for logging.
    pub fn name(self) -> &'static str {
        match self {
            MdState::None => "none",
            MdState::Pairing => "pairing",
            MdState::Connected => "connected",
            MdState::Disconnected => "disconnected",
            MdState::Reject => "reject",
        }
    }
}

// ===========================================================================
// Shared MdInfo (md_info_t, module.c:60-77)
// ===========================================================================

/// Live connection state (`md_info.state`), defaulting to `MD_STATE_NONE`.
static MD_STATE: AtomicU8 = AtomicU8::new(0);
/// Host LED indicator bitmap (`md_info.indicator`).
static MD_INDICATOR: AtomicU8 = AtomicU8::new(0);
/// Radio firmware version (`md_info.version`).
static MD_VERSION: AtomicU8 = AtomicU8::new(0);
/// Battery level 0..=100 (`md_info.bat`), defaulting to 100 like the vendor.
static MD_BAT: AtomicU8 = AtomicU8::new(100);

/// Current radio connection state.
pub fn state() -> MdState {
    MdState::from_u8(MD_STATE.load(Ordering::Relaxed))
}

/// Latest reported radio firmware version.
pub fn version() -> u8 {
    MD_VERSION.load(Ordering::Relaxed)
}

/// Latest reported battery level (0..=100).
pub fn battery() -> u8 {
    MD_BAT.load(Ordering::Relaxed)
}

/// Force the connection state, bypassing a `DEVCTRL` notification. The
/// connection state machine uses this to drop to `MD_STATE_DISCONNECTED` on a
/// device switch / pairing reset (`wireless_devs_change`, wireless.c:197-200).
pub fn set_state(s: MdState) {
    MD_STATE.store(s.as_u8(), Ordering::Relaxed);
}

/// Force the host LED indicator bitmap (cleared to `0` alongside
/// [`set_state`] on a device switch, wireless.c:199).
pub fn set_indicator(v: u8) {
    MD_INDICATOR.store(v, Ordering::Relaxed);
}

/// Fold a decoded event into [`MdInfo`](self) — the side effects of the vendor
/// `md_receive_msg_task` switch (`module.c:199-225`). `RawOut`/`HostState` carry
/// no persistent state and are routed elsewhere.
pub fn apply_event(ev: &MdEvent) {
    match ev {
        MdEvent::Indicator(v) => MD_INDICATOR.store(*v, Ordering::Relaxed),
        MdEvent::DevCtrl(st) => MD_STATE.store(st.as_u8(), Ordering::Relaxed),
        MdEvent::BatVol(v) => MD_BAT.store(*v, Ordering::Relaxed),
        MdEvent::FwVersion(v) => MD_VERSION.store(*v, Ordering::Relaxed),
        MdEvent::RawOut(_) | MdEvent::HostState(_) => {}
    }
}

// ===========================================================================
// Decoded inbound events
// ===========================================================================

/// A decoded, checksum-valid inbound frame's payload.
#[derive(Clone, Copy)]
pub enum MdEvent {
    /// Raw-HID report from the host (`MD_REV_CMD_RAW`), exactly 32 bytes.
    RawOut([u8; MD_RAW_SIZE]),
    /// Host keyboard-LED indicator state (`MD_REV_CMD_INDICATOR`).
    Indicator(u8),
    /// Connection-state change (`MD_REV_CMD_DEVCTRL`).
    DevCtrl(MdState),
    /// Battery level 0..=100 (`MD_REV_CMD_BATVOL`).
    BatVol(u8),
    /// Radio firmware version (`MD_REV_CMD_MD_FW_VERSION`).
    FwVersion(u8),
    /// Host suspend/resume (`MD_REV_CMD_HOST_STATE`); `true` = resume.
    HostState(bool),
}

/// The result of feeding one byte to [`MdRx::push`].
pub enum MdStep {
    /// Still mid-frame (or the byte was dropped); nothing to do.
    Pending,
    /// The 3-byte sync ACK (`61 0D 0A`) arrived: the radio acknowledged our
    /// in-flight frame. The caller frees the stop-and-wait slot and echoes
    /// **no** ACK back.
    AckToken,
    /// A complete, checksum-valid frame was decoded. The caller echoes the
    /// 3-byte ACK to the radio; `0` carries the decoded event, if any.
    Frame(Option<MdEvent>),
}

// ===========================================================================
// Checksum (module.c:85-103)
// ===========================================================================

/// 8-bit additive sum of `bytes` (`md_calc_check_sum`/`md_check_sum`).
fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0u8, |sum, &b| sum.wrapping_add(b))
}

// ===========================================================================
// Outbound frame builders (md_send_*, module.c:323-455)
// ===========================================================================

/// A built outbound frame: the framed bytes plus their length. Fits the largest
/// frame this codec produces ([`FRAME_MAX`]).
#[derive(Clone, Copy)]
pub struct Frame {
    bytes: [u8; FRAME_MAX],
    len: u8,
}

impl Frame {
    const fn empty() -> Self {
        Self {
            bytes: [0; FRAME_MAX],
            len: 0,
        }
    }

    /// Build `[opcode][payload…][checksum]`, appending the additive checksum
    /// over the opcode and payload — the shape shared by every `md_send_*`
    /// builder whose checksum is the final byte.
    fn framed(opcode: u8, payload: &[u8]) -> Self {
        let mut f = Self::empty();
        f.bytes[0] = opcode;
        let body = 1 + payload.len();
        f.bytes[1..body].copy_from_slice(payload);
        f.bytes[body] = checksum(&f.bytes[..body]);
        f.len = (body + 1) as u8;
        f
    }

    /// The framed bytes ready for the UART.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..self.len as usize]
    }

    /// `md_send_kb` — 8-byte boot keyboard report (module.c:323-330).
    pub fn keyboard(report: &[u8; MD_SND_CMD_KB_LEN]) -> Self {
        Self::framed(MD_SND_CMD_SEND_KB, report)
    }

    /// `md_send_nkro` — 14-byte NKRO bitmap (module.c:332-339).
    pub fn nkro(report: &[u8; MD_SND_CMD_NKRO_LEN]) -> Self {
        Self::framed(MD_SND_CMD_SEND_NKRO, report)
    }

    /// `md_send_consumer` — 2-byte consumer usage, little-endian
    /// (module.c:341-348).
    pub fn consumer(usage: u16) -> Self {
        // The fixed-length binding compile-time-guarantees the 2-byte consumer
        // payload matches `MD_SND_CMD_CONSUMER_LEN`, replacing a runtime assert.
        let payload: [u8; MD_SND_CMD_CONSUMER_LEN] = usage.to_le_bytes();
        Self::framed(MD_SND_CMD_SEND_CONSUMER, &payload)
    }

    /// `md_send_system` — 1-byte system-control bitmask (module.c:350-357).
    pub fn system(bitmask: u8) -> Self {
        let payload = [bitmask; MD_SND_CMD_SYSTEM_LEN];
        Self::framed(MD_SND_CMD_SEND_SYSTEM, &payload)
    }

    /// `md_send_mouse` — 5-byte relative mouse report (module.c:361-368). The wire
    /// payload is `[buttons, x, y, h, v]`, matching the vendor `wls_report_mouse_t`
    /// (wireless.c:132-157): the QMK report's horizontal wheel (`report->h`) lands
    /// in byte 3 and its vertical wheel (`report->v`) in byte 4. keeberry's mouse
    /// keys scroll only vertically, so the caller passes `h = 0` and `v` = the wheel.
    pub fn mouse(buttons: u8, x: i8, y: i8, h: i8, v: i8) -> Self {
        let payload: [u8; MD_SND_CMD_MOUSE_LEN] =
            [buttons, x as u8, y as u8, h as u8, v as u8];
        Self::framed(MD_SND_CMD_SEND_MOUSE, &payload)
    }

    /// `md_send_raw` — raw-HID frame `[0xAF][0x61][32][32 bytes][checksum]`
    /// (module.c:442-455). The length byte (32) sits at index 2; the checksum
    /// is the final byte.
    pub fn raw(data: &[u8; MD_RAW_SIZE]) -> Self {
        let mut payload = [0u8; 2 + MD_RAW_SIZE];
        payload[0] = MD_SND_CMD_RAW_IN;
        payload[1] = MD_RAW_SIZE as u8;
        payload[2..].copy_from_slice(data);
        Self::framed(MD_SND_CMD_RAW, &payload)
    }

    /// `md_send_devctrl` — device-control `[0xA6][sub][checksum]`
    /// (module.c:393-400).
    pub fn devctrl(sub: u8) -> Self {
        Self::framed(MD_SND_CMD_DEVCTRL, &[sub])
    }

    /// `md_send_vpid` — `[0xAD] + ((pid << 16) | vid) little-endian + checksum`
    /// (module.c:430-440).
    pub fn vpid(vid: u16, pid: u16) -> Self {
        let vpid: u32 = ((pid as u32) << 16) | (vid as u32);
        Self::framed(MD_SND_CMD_VPID, &vpid.to_le_bytes())
    }

    /// `md_send_devinfo` — device-name advertisement `[0xA9][len][name]…`
    /// (module.c:377-391). Returns `None` if `name` exceeds
    /// [`MD_SND_CMD_DEVINFO_LEN`]. Faithful to the vendor, the checksum is
    /// written at index `len + 2` and the frame is always padded out to
    /// `DEVINFO_LEN + 3` bytes (the `sizeof(sdata)` the vendor pushes), so the
    /// checksum is *not* the final byte.
    pub fn devinfo(name: &[u8]) -> Option<Self> {
        if name.len() > MD_SND_CMD_DEVINFO_LEN {
            return None;
        }
        let mut f = Self::empty();
        f.bytes[0] = MD_SND_CMD_SEND_DEVINFO;
        f.bytes[1] = name.len() as u8;
        f.bytes[2..2 + name.len()].copy_from_slice(name);
        f.bytes[name.len() + 2] = checksum(&f.bytes[..name.len() + 2]);
        f.len = (MD_SND_CMD_DEVINFO_LEN + 3) as u8;
        Some(f)
    }

    /// `[opcode][len][str][checksum]` dongle-string frame, pushed at its exact
    /// `len + 3` length (no padding, unlike [`devinfo`](Self::devinfo)) — the
    /// shape shared by `md_send_manufacturer`/`md_send_product`
    /// (module.c:402-428).
    ///
    /// `name` is always ASCII (the
    /// [`Devs::device_name`](crate::wireless::Devs::device_name) and
    /// [`MANUFACTURER`](crate::usb::MANUFACTURER) literals); each byte is widened to
    /// a UTF-16LE code unit (`b` → `[b, 0x00]`) so the dongle, which copies the bytes
    /// verbatim into its UTF-16LE USB string descriptor, renders the name correctly
    /// rather than as CJK mojibake. The length byte is the *on-wire* byte count
    /// (twice the character count), which is what the dongle needs to size the
    /// descriptor.
    ///
    /// The vendor protocol permits up to 46-byte strings
    /// (`MD_SND_CMD_MANUFACTURER_LEN`/`_PRODUCT_LEN`, module.h:33-34), but this
    /// codec's [`Frame`] buffer is sized for the 36-byte raw-HID frame
    /// ([`FRAME_MAX`]); a dongle string is therefore bounded to
    /// [`DONGLE_STRING_MAX`] (`FRAME_MAX - 3`, for the opcode, length byte and
    /// checksum) on its widened length, which the short keeberry strings fit with
    /// room to spare. A longer string is rejected (`None`, the frame is dropped)
    /// rather than overflowing the fixed buffer — so no input can panic.
    fn dongle_string(opcode: u8, s: &[u8]) -> Option<Self> {
        // Widen ASCII → UTF-16LE ('A' 0x41 → [0x41, 0x00]); correct for these
        // ASCII-only names (each < U+0080 is a single UTF-16 code unit). The on-wire
        // length is therefore two bytes per character.
        let wire_len = s.len() * 2;
        if wire_len > DONGLE_STRING_MAX {
            return None;
        }
        let mut f = Self::empty();
        f.bytes[0] = opcode;
        f.bytes[1] = wire_len as u8;
        for (i, &b) in s.iter().enumerate() {
            f.bytes[2 + i * 2] = b;
            f.bytes[2 + i * 2 + 1] = 0;
        }
        f.bytes[wire_len + 2] = checksum(&f.bytes[..wire_len + 2]);
        f.len = (wire_len + 3) as u8;
        Some(f)
    }

    /// `md_send_manufacturer` — the dongle's USB manufacturer string
    /// (module.c:402-414). Used only in the 2.4G pairing sequence; pushed as
    /// UTF-16LE (see [`Frame::dongle_string`]).
    pub fn manufacturer(name: &[u8]) -> Option<Self> {
        Self::dongle_string(MD_SND_CMD_MANUFACTURER, name)
    }

    /// `md_send_product` — the dongle's USB product string (module.c:416-428).
    /// Used only in the 2.4G pairing sequence; pushed as UTF-16LE (see
    /// [`Frame::dongle_string`]).
    pub fn product(name: &[u8]) -> Option<Self> {
        Self::dongle_string(MD_SND_CMD_PRODUCT, name)
    }
}

// ===========================================================================
// Receive state machine (md_receive_msg_task, module.c:121-235)
// ===========================================================================

/// Receive buffer length. The vendor `md_rev_payload` is `MD_SEND_PKT_PAYLOAD_MAX`
/// (36) bytes; a couple of extra slots give the overflow guard headroom.
const RX_BUF_LEN: usize = FRAME_MAX + 4;

/// Byte-at-a-time receive state machine. A faithful port of the `data_count` /
/// `data_remain` switch in `md_receive_msg_task`, restructured so each call
/// consumes one byte and yields an [`MdStep`] instead of looping on
/// `uart_available()` internally.
pub struct MdRx {
    buf: [u8; RX_BUF_LEN],
    /// `data_count`: bytes accepted into the current frame.
    count: usize,
    /// `data_remain`: bytes still expected before the frame is complete.
    remain: u8,
}

impl MdRx {
    /// A fresh receiver at the start-of-frame state.
    pub const fn new() -> Self {
        Self {
            buf: [0; RX_BUF_LEN],
            count: 0,
            remain: 0,
        }
    }

    /// Reset to the start-of-frame state (`data_count = 0`).
    fn reset(&mut self) {
        self.count = 0;
        self.remain = 0;
    }

    /// Feed one received byte; see [`MdStep`].
    pub fn push(&mut self, data: u8) -> MdStep {
        match self.count {
            0 => {
                // Opcode byte: accept only the known inbound opcodes (plus the
                // ACK's leading 0x61), otherwise drop and resync
                // (module.c:129-146).
                match data {
                    MD_REV_CMD_RAW
                    | MD_REV_CMD_INDICATOR
                    | MD_REV_CMD_DEVCTRL
                    | MD_REV_CMD_BATVOL
                    | MD_REV_CMD_MD_FW_VERSION
                    | MD_REV_CMD_HOST_STATE
                    | 0x61 => {
                        self.buf[0] = data;
                        self.count = 1;
                        self.remain = 2;
                    }
                    _ => self.count = 0,
                }
                MdStep::Pending
            }
            1 => {
                // Second byte (module.c:147-151).
                self.buf[1] = data;
                self.count = 2;
                self.remain = self.remain.wrapping_sub(1);
                MdStep::Pending
            }
            2 => {
                // Sync ACK `61 0D 0A` (module.c:153-160).
                if self.buf[0] == 0x61 && self.buf[1] == 0x0D && data == 0x0A {
                    self.reset();
                    return MdStep::AckToken;
                }
                // Raw-HID: the length byte follows, then that many bytes plus a
                // checksum (module.c:162-167).
                if self.buf[0] == MD_REV_CMD_RAW && self.buf[1] == MD_REV_CMD_RAW_OUT {
                    self.buf[2] = data;
                    self.count = 3;
                    self.remain = data.wrapping_add(1);
                    return MdStep::Pending;
                }
                // Otherwise this byte is the checksum of a 3-byte frame: the C
                // `case 2` falls through into `default` (module.c:168-176).
                self.push_default(data)
            }
            _ => self.push_default(data),
        }
    }

    /// The C switch `default` arm (module.c:169-176): store the byte, and when
    /// no bytes remain, validate and decode the frame.
    fn push_default(&mut self, data: u8) -> MdStep {
        if self.count >= self.buf.len() {
            // The vendor lacks this guard; a malformed raw length could overrun
            // `md_rev_payload`. Drop the frame and resync instead.
            self.reset();
            return MdStep::Pending;
        }
        self.buf[self.count] = data;
        self.count += 1;
        self.remain = self.remain.wrapping_sub(1);
        if self.remain != 0 {
            return MdStep::Pending;
        }
        let step = self.decode();
        self.reset();
        step
    }

    /// Validate the checksum and decode a complete frame (module.c:179-231).
    /// Pure: state writes happen in [`apply_event`] after the caller echoes the
    /// ACK, matching the vendor's "ACK then update" order.
    fn decode(&self) -> MdStep {
        let len = self.count;
        // md_check_sum: sum of bytes[..len-1] equals the trailing byte.
        if checksum(&self.buf[..len - 1]) != self.buf[len - 1] {
            // Invalid: drop silently, no ACK (the vendor just resets data_count).
            return MdStep::Pending;
        }

        let event = match self.buf[0] {
            MD_REV_CMD_RAW => {
                // A real raw-HID payload exists only in a full
                // `AF 60 20 <32 bytes> <checksum>` frame (length 36). A shorter
                // 0xAF-led frame that merely passes the additive checksum — e.g.
                // `AF 71 20`, where 0xAF + 0x71 = 0x120 → 0x20 == buf[2] — must
                // NOT dispatch stale bytes from buf[3..]. Gate on the RAW_OUT
                // sub-opcode and the full frame length, not just buf[2] == 32:
                // the vendor only reaches the long-frame path when
                // buf[1] == RAW_OUT (module.c:163) and only copies when
                // len == 32 (module.c:194). The state machine still ACKs any
                // valid-checksum frame (vendor-faithful); only the RawOut
                // dispatch is guarded, so a bogus 0xAF frame yields no event.
                if self.buf[1] == MD_REV_CMD_RAW_OUT
                    && len == FRAME_MAX
                    && self.buf[2] as usize == MD_RAW_SIZE
                {
                    let mut d = [0u8; MD_RAW_SIZE];
                    d.copy_from_slice(&self.buf[3..3 + MD_RAW_SIZE]);
                    Some(MdEvent::RawOut(d))
                } else {
                    None
                }
            }
            MD_REV_CMD_INDICATOR => Some(MdEvent::Indicator(self.buf[1])),
            MD_REV_CMD_DEVCTRL => match self.buf[1] {
                MD_REV_CMD_DEVCTRL_PAIRING => Some(MdEvent::DevCtrl(MdState::Pairing)),
                MD_REV_CMD_DEVCTRL_CONNECTED => Some(MdEvent::DevCtrl(MdState::Connected)),
                MD_REV_CMD_DEVCTRL_DISCONNECTED => Some(MdEvent::DevCtrl(MdState::Disconnected)),
                MD_REV_CMD_DEVCTRL_REJECT => Some(MdEvent::DevCtrl(MdState::Reject)),
                // Unknown sub-command: still a valid frame (ACK is echoed), but
                // no state change (the vendor switch `default`).
                _ => None,
            },
            MD_REV_CMD_BATVOL => Some(MdEvent::BatVol(self.buf[1])),
            MD_REV_CMD_MD_FW_VERSION => Some(MdEvent::FwVersion(self.buf[1])),
            MD_REV_CMD_HOST_STATE => {
                Some(MdEvent::HostState(self.buf[1] == MD_REV_CMD_HOST_STATE_RESUME))
            }
            _ => None,
        };

        MdStep::Frame(event)
    }
}
