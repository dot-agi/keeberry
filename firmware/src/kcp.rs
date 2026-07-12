// SPDX-License-Identifier: GPL-2.0-or-later
//! keeberry config protocol (kcp) — framing, dispatch and every command group.
//!
//! kcp is keeberry's single configuration surface: a fixed-size, zero-alloc
//! binary protocol carried over a raw-HID vendor interface (usage page
//! `0xFF60`, usage `0x61`; built in [`crate::usb`]). The bytes are identical
//! over every link — the stock 2.4 GHz dongle bridges exactly this usage page —
//! so kcp runs unchanged over USB and, via the dongle bridge, over 2.4 GHz / BT.
//! It is deliberately *not* Vial: the descriptor is only the pipe, the payload is
//! 100 % kcp.
//!
//! # Framing
//!
//! One 32-byte HID report is exactly one message; there is no report ID.
//!
//! ```text
//! request : [0]=CMD       [1]=SEQ  [2..32]=payload (30 bytes)
//! reply   : [0]=CMD|0x80  [1]=SEQ  [2]=STATUS  [3..32]=payload (29 bytes)
//! ```
//!
//! * `CMD` high nibble selects the command [`group`] (INFO `0x0x`, KEYMAP
//!   `0x1x`, …); the low nibble selects the operation within that group.
//! * `SEQ` is an opaque host-chosen tag, echoed verbatim so the host can pair a
//!   reply with its request (and key chunked multi-report exchanges, as MACRO does).
//! * `STATUS` ([`Status`]) reports the outcome; it is present only in replies.
//! * Replies set bit 7 of `CMD` ([`REPLY_FLAG`]). For groups `0x0x`–`0x7x` that
//!   alone distinguishes a reply from an echoed request; groups `0x8x`–`0xFx`
//!   already have bit 7 set, so for those the echoed `SEQ` is what pairs reply
//!   to request. All thirteen command groups are implemented (eleven without the
//!   optional TEXT and UNICODE groups).
//!
//! Large transfers are chunked over the payload and `SEQ` where needed — the
//! MACRO group uploads one step per report; single-report groups like INFO need none.
//!
//! # Dispatch
//!
//! [`handle`] is a total function `&[u8; 32] -> [u8; 32]`: every input yields a
//! reply and it neither allocates nor blocks, so it is straightforward to reason
//! about and to test. A handler may still read live device state — the KEYMAP
//! group reads the RAM keymap, the TELEMETRY group the monotonic clock and the
//! telemetry counters — so its reply reflects the moment it is built. It routes
//! on the CMD group to a per-group handler that fills the reply payload and
//! returns a [`Status`]. Each group is a single match arm in [`handle`] with one
//! bit in [`CAPABILITIES`]. An unknown group — a nibble this firmware does not
//! assign — replies [`Status::Unsupported`], which is
//! also how the host negotiates capabilities (see [`CMD_GET_CAPABILITIES`]): the
//! GUI shows only the groups the firmware reports and never breaks on an unknown
//! command.

use crate::config;
use crate::features;
use crate::keycode::Keycode;
use crate::keymap::{self, LAYERS};
use crate::matrix::{self, NUM_COLS, NUM_ROWS};
use crate::rgb;
use crate::telemetry;
use crate::timed;
use crate::wireless;
use embassy_time::Instant;

/// Length of one kcp message: a single 32-byte HID report (IN and OUT).
pub const MSG_LEN: usize = 32;

/// Bit OR-ed into the `CMD` byte to mark a frame as a reply.
pub const REPLY_FLAG: u8 = 0x80;

// Byte offsets within a frame. Requests have no STATUS byte, so their payload
// starts at index 2; replies insert STATUS at index 2 and start payload at 3.
const CMD_IDX: usize = 0;
const SEQ_IDX: usize = 1;
const STATUS_IDX: usize = 2;
/// First payload byte of a *reply* (after CMD, SEQ, STATUS).
const REPLY_PAYLOAD_IDX: usize = 3;
/// First payload byte of a *request* (after CMD, SEQ).
const REQ_PAYLOAD_IDX: usize = 2;

/// Result of handling a request, returned to the host in reply byte `[2]`.
///
/// The numbering is fixed by the kcp spec. [`Status::Busy`] is returned by the
/// CONFIG group when a flash save fails. Live edits return [`Status::Ok`] as soon
/// as the RAM change takes effect; persisting to flash is a separate, explicit
/// `CONFIG.SAVE`, and tracking which RAM edits are still unsaved is the GUI's job.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum Status {
    /// Request handled successfully; any result is in the reply payload.
    Ok = 0,
    /// The command code is not a valid operation of its (known) group.
    BadCmd = 1,
    /// A payload argument was missing or out of range.
    BadArg = 2,
    /// The device is busy and cannot service the request right now.
    Busy = 3,
    /// The command's group is not implemented by this firmware (capability
    /// negotiation: absent feature). Returned for any unknown group.
    Unsupported = 4,
}

/// Command groups: the high nibble of `CMD`, and the bit index of the group in
/// the [`CAPABILITIES`] bitmask (so present group `g` sets bit `g`).
///
/// Every firmware feature is a group. All are wired into [`handle`] and advertised
/// in [`CAPABILITIES`]; the high nibble of a request's `CMD` selects the group, the
/// low nibble the operation within it. TEXT and UNICODE are the optional groups —
/// each is wired and advertised only when its owning Cargo feature is built in (a
/// `--no-default-features` build drops them). FEATURES is always present: it
/// enumerates the registry, whose structural core ships in every build.
pub mod group {
    /// `0x0x` — protocol version, capabilities and device info.
    pub const INFO: u8 = 0x0;
    /// `0x1x` — live keymap get/set: keycode, layer count, layer config.
    pub const KEYMAP: u8 = 0x1;
    /// `0x2x` — telemetry (latency, scan/report rate, battery, link, layer).
    pub const TELEMETRY: u8 = 0x2;
    /// `0x3x` — rollover mode get/set (boot 6KRO / NKRO).
    pub const HID_KRO: u8 = 0x3;
    /// `0x4x` — config: save, restore defaults, storage info, debounce and runtime tuning.
    pub const CONFIG: u8 = 0x4;
    /// `0x5x` — macros: info, get/set step, clear, play, record start/stop.
    pub const MACRO: u8 = 0x5;
    /// `0x6x` — RGB: mode, HSV, brightness, on/off, effect list, zones.
    pub const RGB: u8 = 0x6;
    /// `0x7x` — behaviours: tap-dance, combos, key-overrides, SOCD.
    pub const BEHAVIOR: u8 = 0x7;
    /// `0x8x` — wireless: state, mode, pair/unpair, sleep, battery.
    pub const WIRELESS: u8 = 0x8;
    /// `0x9x` — text input: autocorrect enable / info. Defined only with the `autocorrect`
    /// feature that owns the group, so a build without it neither names nor routes it.
    #[cfg(feature = "autocorrect")]
    pub const TEXT: u8 = 0x9;
    /// `0xAx` — Unicode input: active OS mode + the host-uploaded codepoint map. Gated on
    /// the `unicode` Cargo feature, like every reference to it (the capability bit and the
    /// dispatch arm), so a `--no-default-features` build defines no group it cannot answer.
    #[cfg(feature = "unicode")]
    pub const UNICODE: u8 = 0xA;
    /// `0xDx` — features: enumerate every registered feature and toggle its runtime
    /// enable. Registry-owned (the dispatcher iterates [`crate::features::FEATURES`]),
    /// always present — the structural feature core is built into every configuration.
    pub const FEATURES: u8 = 0xD;
    // @scaffold:kcp-group — `just new-feature <Name> --kind config` inserts a new
    // feature-owned group const `pub const <NAME>: u8 = 0x<nibble>;` here, taking the
    // lowest free nibble (`0xB`, `0xC` or `0xE`); pair it with a `CAPABILITIES` bit and a
    // dispatch arm at `// @scaffold:kcp-dispatch`.
    /// `0xFx` — system: enter DFU, reboot, USB mode, digitizer.
    pub const SYSTEM: u8 = 0xF;
}

/// The UNICODE capability bit, gated on the `unicode` Cargo feature: `1 << group::UNICODE`
/// when the feature is on, `0` when it is off (a `--no-default-features` build). Paired with
/// the cfg-gated [`group::UNICODE`] dispatch arm in [`handle`], so a unicode-less build
/// neither advertises bit 10 nor answers group `0xA`.
#[cfg(feature = "unicode")]
const UNICODE_CAP: u32 = 1 << group::UNICODE;
#[cfg(not(feature = "unicode"))]
const UNICODE_CAP: u32 = 0;

/// Bitmask of command groups this firmware implements, returned by
/// [`CMD_GET_CAPABILITIES`] as a little-endian `u32`.
///
/// Bit `g` corresponds to [`group`] `g`: bit 0 = INFO, bit 1 = KEYMAP, …,
/// bit 9 = TEXT, bit 10 = UNICODE, bit 13 = FEATURES, bit 15 = SYSTEM. INFO, KEYMAP,
/// TELEMETRY, HID_KRO, CONFIG, MACRO, RGB, BEHAVIOR, WIRELESS, FEATURES and SYSTEM are
/// always present; TEXT (bit 9) is owned by the `autocorrect` feature
/// ([`TEXT_CAPABILITY`]) and UNICODE (bit 10) by the `unicode` feature ([`UNICODE_CAP`]).
/// So the default build (both features on) advertises `0xA7FF`, while a
/// `--no-default-features` build advertises `0xA1FF` (and the GUI, seeing a bit clear,
/// hides that feature's panel).
const CAPABILITIES: u32 = (1 << group::INFO)
    | (1 << group::KEYMAP)
    | (1 << group::TELEMETRY)
    | (1 << group::HID_KRO)
    | (1 << group::CONFIG)
    | (1 << group::MACRO)
    | (1 << group::RGB)
    | (1 << group::BEHAVIOR)
    | (1 << group::WIRELESS)
    | TEXT_CAPABILITY
    | UNICODE_CAP
    | (1 << group::FEATURES)
    | (1 << group::SYSTEM);

/// The TEXT group's capability bit, present only when the `autocorrect` feature — which owns
/// the group — is built in. A build without it neither advertises bit 9 nor routes the group,
/// so [`handle`]'s TEXT arm and the `CMD_TEXT_*` opcodes are `#[cfg]`-gated to match.
#[cfg(feature = "autocorrect")]
const TEXT_CAPABILITY: u32 = 1 << group::TEXT;
#[cfg(not(feature = "autocorrect"))]
const TEXT_CAPABILITY: u32 = 0;

// === INFO group (0x0x) =====================================================

/// INFO `0x00` — get the kcp protocol version. Reply payload = `[major, minor]`
/// ([`PROTOCOL_VERSION`]).
pub const CMD_GET_VERSION: u8 = 0x00;
/// INFO `0x01` — get the capabilities bitmask. Reply payload = [`CAPABILITIES`]
/// as a little-endian `u32`.
pub const CMD_GET_CAPABILITIES: u8 = 0x01;
/// INFO `0x02` — get device info (firmware/chip/matrix/transport plus the config
/// schema version). Reply payload layout is documented on [`pack_device_info`].
pub const CMD_GET_DEVICE_INFO: u8 = 0x02;

/// kcp protocol version `[major, minor]`, reported by [`CMD_GET_VERSION`].
pub const PROTOCOL_VERSION: [u8; 2] = [0, 2];

/// Firmware version `[major, minor, patch]`, part of [`CMD_GET_DEVICE_INFO`].
pub const FIRMWARE_VERSION: [u8; 3] = [0, 1, 0];

/// Chip identifier reported by [`CMD_GET_DEVICE_INFO`] — 8 ASCII bytes.
const CHIP_ID: &[u8; 8] = b"WB32FQ95";

// Compile-time guarantees for the casts and copies below: the device-info
// fields must fit a byte each, and the packed descriptor must fit the reply.
const _: () = assert!(NUM_ROWS <= u8::MAX as usize, "matrix rows must fit a u8");
const _: () = assert!(NUM_COLS <= u8::MAX as usize, "matrix cols must fit a u8");
const _: () = assert!(LAYERS <= u8::MAX as usize, "layer count must fit a u8");
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= DEVICE_INFO_LEN,
    "device-info payload must fit one reply"
);

/// Total length of the packed [`CMD_GET_DEVICE_INFO`] payload (see
/// [`pack_device_info`]).
const DEVICE_INFO_LEN: usize = 17;

/// Dispatch a 32-byte request to its group handler and build the 32-byte reply.
///
/// Total: every input yields a reply, with no allocation and no blocking. The
/// reply echoes `CMD` with [`REPLY_FLAG`] set and the request's `SEQ`, then a
/// [`Status`] and an (already zero-initialised) payload the handler may fill.
/// Unknown groups get [`Status::Unsupported`].
#[must_use]
pub fn handle(req: &[u8; MSG_LEN]) -> [u8; MSG_LEN] {
    let cmd = req[CMD_IDX];
    let seq = req[SEQ_IDX];

    let mut reply = [0u8; MSG_LEN];
    reply[CMD_IDX] = cmd | REPLY_FLAG;
    reply[SEQ_IDX] = seq;

    let req_payload = &req[REQ_PAYLOAD_IDX..];
    let out = &mut reply[REPLY_PAYLOAD_IDX..];

    // Route on the command group (CMD high nibble): each group is one arm here,
    // its presence advertised by the matching bit in CAPABILITIES. The MACRO,
    // BEHAVIOR, TEXT and UNICODE groups are owned by registry features (the timed
    // engine, SOCD, overrides, autocorrect and unicode), so they route through
    // `features::run_on_kcp` — the first feature to claim `cmd` answers it, and an
    // unrecognised op within a feature-owned group still returns BadCmd (the feature
    // reports it), preserving every reply.
    let status = match cmd >> 4 {
        group::INFO => info_dispatch(cmd, req_payload, out),
        group::KEYMAP => keymap_dispatch(cmd, req_payload, out),
        group::TELEMETRY => telemetry_dispatch(cmd, req_payload, out),
        group::HID_KRO => hid_kro_dispatch(cmd, req_payload, out),
        group::CONFIG => config_dispatch(cmd, req_payload, out),
        group::MACRO => features::run_on_kcp(cmd, req_payload, out),
        group::RGB => rgb_dispatch(cmd, req_payload, out),
        group::BEHAVIOR => features::run_on_kcp(cmd, req_payload, out),
        group::WIRELESS => wireless_dispatch(cmd, req_payload, out),
        // TEXT is owned by the `autocorrect` feature; without it the arm vanishes and a TEXT
        // request falls through to `Unsupported`, matching the cleared capability bit.
        #[cfg(feature = "autocorrect")]
        group::TEXT => features::run_on_kcp(cmd, req_payload, out),
        // UNICODE is owned by the `unicode` feature; gated like TEXT, so without it the arm
        // vanishes and the request falls through to `Unsupported`, matching the cleared bit.
        #[cfg(feature = "unicode")]
        group::UNICODE => features::run_on_kcp(cmd, req_payload, out),
        // FEATURES is registry-owned but, unlike the feature-claimed groups above, has a
        // single dispatcher that enumerates the whole registry rather than first-claims.
        group::FEATURES => features::features_dispatch(cmd, req_payload, out),
        // @scaffold:kcp-dispatch — `just new-feature <Name> --kind config` inserts a new
        // feature-owned group's route here, `group::<NAME> => features::run_on_kcp(cmd,
        // req_payload, out),` (first-claims dispatch into the owning feature's `on_kcp`).
        group::SYSTEM => system_dispatch(cmd, req_payload, out),
        _ => Status::Unsupported,
    };

    reply[STATUS_IDX] = status as u8;
    reply
}

/// INFO group handler. `out` is the reply payload region (`reply[3..32]`),
/// zeroed by [`handle`]; the request payload is unused by INFO.
fn info_dispatch(cmd: u8, _req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_GET_VERSION => {
            out[..PROTOCOL_VERSION.len()].copy_from_slice(&PROTOCOL_VERSION);
            Status::Ok
        }
        CMD_GET_CAPABILITIES => {
            out[..4].copy_from_slice(&CAPABILITIES.to_le_bytes());
            Status::Ok
        }
        CMD_GET_DEVICE_INFO => {
            pack_device_info(out);
            Status::Ok
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

/// Pack the device descriptor for [`CMD_GET_DEVICE_INFO`] into the reply payload.
/// The fields are static save for byte 14, the live transport (kept consistent
/// with the TELEMETRY group's connection byte). Layout, with payload-relative byte
/// offsets:
///
/// | bytes   | field                                                       |
/// |---------|-------------------------------------------------------------|
/// | `0..3`  | firmware version `[major, minor, patch]`                    |
/// | `3..11` | chip id, 8 ASCII bytes (`"WB32FQ95"`)                        |
/// | `11`    | matrix rows ([`NUM_ROWS`])                                  |
/// | `12`    | matrix columns ([`NUM_COLS`])                               |
/// | `13`    | layer count ([`LAYERS`])                                    |
/// | `14`    | transport / connection (live [`wireless::Devs`] code, USB = 0) |
/// | `15..17`| config schema version ([`config::SCHEMA_VERSION`], `u16` LE)|
///
/// Total [`DEVICE_INFO_LEN`] bytes. `out` must be at least that long, which the
/// module-level `const` assertions guarantee for any reply payload. The schema
/// version lets the host tell whether a backed-up config blob is restorable into
/// this firmware after a flash (it is the version this firmware reads/writes).
fn pack_device_info(out: &mut [u8]) {
    out[0..3].copy_from_slice(&FIRMWARE_VERSION);
    out[3..11].copy_from_slice(CHIP_ID);
    out[11] = NUM_ROWS as u8;
    out[12] = NUM_COLS as u8;
    out[13] = LAYERS as u8;
    out[14] = wireless::transport().code();
    out[15..17].copy_from_slice(&config::SCHEMA_VERSION.to_le_bytes());
}

// === KEYMAP group (0x1x) ===================================================
//
// Live keymap editing. Each operation reads or writes the mutable RAM keymap in
// [`crate::keymap`], so — unlike the pure INFO group — this handler legitimately
// touches device state. Editing is per-key (GET/SET_KEYCODE) by design: one row is
// `NUM_COLS * 2 = 30` bytes, past the 29-byte reply payload, so a whole-layer dump
// would need chunking the GUI does not require — it edits key by key.

/// KEYMAP `0x10` — get the keycode bound at a matrix position.
///
/// Request payload `[layer, row, col]`; reply payload is the [`Keycode`] as a
/// little-endian `u16` (`[kc_lo, kc_hi]`). An out-of-range `(layer, row, col)`
/// replies [`Status::BadArg`].
pub const CMD_GET_KEYCODE: u8 = 0x10;

/// KEYMAP `0x11` — set the keycode bound at a matrix position.
///
/// Request payload `[layer, row, col, kc_lo, kc_hi]` (the keycode little-endian).
/// The binding is applied *live* to the RAM keymap and takes effect on the next
/// scan; an out-of-range position replies [`Status::BadArg`], otherwise
/// [`Status::Ok`]. The edit is RAM-live; the host persists it (with every other
/// group's state) via the CONFIG group's [`CMD_CONFIG_SAVE`].
pub const CMD_SET_KEYCODE: u8 = 0x11;

/// KEYMAP `0x12` — get the number of keymap layers.
///
/// No request payload; reply payload is `[LAYERS]` (a single byte).
pub const CMD_GET_LAYER_COUNT: u8 = 0x12;

/// KEYMAP `0x13` — get the layer configuration (the persistent default layer and the
/// tri-layer rule).
///
/// No request payload; reply payload is `[default_layer, tri_enabled, tri_l1, tri_l2,
/// tri_l3]` ([`LAYER_CONFIG_LEN`]). `default_layer` is the base `DF` layer; the
/// tri-layer fields describe the rule "`l1` and `l2` active ⇒ `l3` active". Always
/// [`Status::Ok`].
pub const CMD_GET_LAYER_CONFIG: u8 = 0x13;

/// KEYMAP `0x14` — set the layer configuration.
///
/// Request payload mirrors [`CMD_GET_LAYER_CONFIG`]: `[default_layer, tri_enabled,
/// tri_l1, tri_l2, tri_l3]`. An out-of-range default layer, or an enabled tri-layer
/// with an out-of-range layer or `l1 == l2`, replies [`Status::BadArg`]; otherwise the
/// change is live on the next scan and [`Status::Ok`]. RAM-only until a
/// [`CMD_CONFIG_SAVE`] persists it (it rides the config blob like the keymap).
pub const CMD_SET_LAYER_CONFIG: u8 = 0x14;

/// Length of the [`CMD_GET_LAYER_CONFIG`] / [`CMD_SET_LAYER_CONFIG`] payload:
/// `default_layer` plus the four tri-layer bytes.
const LAYER_CONFIG_LEN: usize = 5;

const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= LAYER_CONFIG_LEN,
    "layer-config payload must fit one reply"
);

/// KEYMAP group handler. Reads and writes the live RAM keymap in
/// [`crate::keymap`].
///
/// `req_payload` is the request region `req[2..32]` — a fixed 30 bytes, so the
/// few leading argument bytes each operation reads are always present — and
/// `out` is the zeroed reply payload `reply[3..32]`. The only argument failure
/// is an out-of-range `(layer, row, col)`, which the keymap accessors report and
/// this maps to [`Status::BadArg`]; an unrecognised operation is [`Status::BadCmd`].
fn keymap_dispatch(cmd: u8, req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_GET_KEYCODE => {
            let (layer, row, col) = (
                req_payload[0] as usize,
                req_payload[1] as usize,
                req_payload[2] as usize,
            );
            match keymap::get_keycode(layer, row, col) {
                Some(kc) => {
                    out[..2].copy_from_slice(&kc.raw().to_le_bytes());
                    Status::Ok
                }
                None => Status::BadArg,
            }
        }
        CMD_SET_KEYCODE => {
            let (layer, row, col) = (
                req_payload[0] as usize,
                req_payload[1] as usize,
                req_payload[2] as usize,
            );
            let kc = Keycode::from_raw(u16::from_le_bytes([req_payload[3], req_payload[4]]));
            if keymap::set_keycode(layer, row, col, kc) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        CMD_GET_LAYER_COUNT => {
            out[0] = LAYERS as u8;
            Status::Ok
        }
        CMD_GET_LAYER_CONFIG => {
            let (tri_on, l1, l2, l3) = keymap::tri_layer();
            out[0] = keymap::default_layer();
            out[1] = tri_on as u8;
            out[2] = l1;
            out[3] = l2;
            out[4] = l3;
            Status::Ok
        }
        CMD_SET_LAYER_CONFIG => {
            // Validate the default layer and the tri-layer tuple *before* committing
            // either, so an invalid tri-layer can never leave a changed default layer
            // behind: a rejected request is BadArg with zero state change.
            let default = req_payload[0];
            let (tri_on, l1, l2, l3) =
                (req_payload[1] != 0, req_payload[2], req_payload[3], req_payload[4]);
            if keymap::default_layer_valid(default) && keymap::tri_layer_valid(tri_on, l1, l2, l3) {
                keymap::set_default_layer(default);
                keymap::set_tri_layer(tri_on, l1, l2, l3);
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

// === TELEMETRY group (0x2x) ================================================
//
// Live device telemetry: the counters and the last-iteration latency the
// keyboard loop records into [`crate::telemetry`], plus the boot-time clock.
// Unlike the static INFO group these values change continuously, so each reply
// is a fresh snapshot sampled at dispatch time; unlike the KEYMAP group it is
// read-only and never alters device state. Battery is the radio's live level
// (defaulting to 100 with no radio); link RSSI is always `TELEMETRY_UNAVAILABLE`
// because the md radio protocol carries no RSSI.

/// TELEMETRY `0x20` — get a live telemetry snapshot.
///
/// No request payload; the reply payload layout is documented on
/// [`pack_telemetry`]. Always [`Status::Ok`].
pub const CMD_GET_TELEMETRY: u8 = 0x20;

/// Matrix scan rate reported by [`CMD_GET_TELEMETRY`], in hertz: the keyboard
/// loop scans once per millisecond (see [`keyboard_loop`](crate::usb)).
const SCAN_RATE_HZ: u16 = 1000;

/// Sentinel for a telemetry byte field with no value on the current link.
/// Only link RSSI uses it — the md radio protocol carries no RSSI; battery is a
/// live percentage that defaults to 100 (with no radio), never this sentinel.
const TELEMETRY_UNAVAILABLE: u8 = 0xFF;

/// Total length of the packed [`CMD_GET_TELEMETRY`] payload (see
/// [`pack_telemetry`]).
const TELEMETRY_LEN: usize = 23;

// Compile-time guarantee that the snapshot fits one reply payload
// (reply[3..32], i.e. 29 bytes), mirroring the INFO group's device-info check.
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= TELEMETRY_LEN,
    "telemetry payload must fit one reply"
);

/// TELEMETRY group handler. Read-only: it samples the live [`crate::telemetry`]
/// counters and the monotonic clock, touching no device configuration.
///
/// `out` is the zeroed reply payload `reply[3..32]`; the request carries no
/// arguments. An unrecognised operation is [`Status::BadCmd`].
fn telemetry_dispatch(cmd: u8, _req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_GET_TELEMETRY => {
            pack_telemetry(out);
            Status::Ok
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

/// Pack a live telemetry snapshot for [`CMD_GET_TELEMETRY`] into the reply
/// payload. Every multi-byte field is little-endian. Layout, with
/// payload-relative byte offsets:
///
/// | bytes    | field                                                       |
/// |----------|-------------------------------------------------------------|
/// | `0..4`   | uptime since boot, ms (`u32`; wraps after ~49.7 days)       |
/// | `4..8`   | total matrix scans (`u32`)                                  |
/// | `8..12`  | total HID reports written (`u32`)                           |
/// | `12..14` | active-layer bitmask (`u16`, bit `n` = layer `n`)           |
/// | `14..16` | scan rate, Hz (`u16`, [`SCAN_RATE_HZ`])                     |
/// | `16..20` | last-iteration processing time, µs (`u32`)                  |
/// | `20`     | battery percent (radio `md_info.bat`; 100 with no radio)    |
/// | `21`     | link RSSI (always `0xFF` — the md protocol carries none)     |
/// | `22`     | connection / transport (the active [`wireless::Devs`] code) |
///
/// Total [`TELEMETRY_LEN`] (23) bytes — within the 29-byte reply payload, which
/// the module-level `const` assertion guarantees. Uptime is read from
/// [`Instant::now`] and the counters from [`crate::telemetry`] at call time, so
/// the snapshot is current as of the moment the host asked; the `u64 -> u32`
/// uptime narrowing wraps only after ~49.7 days. Battery is the radio's last
/// reported level (defaulting to 100 before any reading / with no radio) and the
/// connection byte is the live transport; RSSI is always unavailable (the md
/// protocol carries none).
fn pack_telemetry(out: &mut [u8]) {
    let uptime_ms = Instant::now().as_millis() as u32;
    out[0..4].copy_from_slice(&uptime_ms.to_le_bytes());
    out[4..8].copy_from_slice(&telemetry::scan_count().to_le_bytes());
    out[8..12].copy_from_slice(&telemetry::report_count().to_le_bytes());
    out[12..14].copy_from_slice(&telemetry::active_layers().to_le_bytes());
    out[14..16].copy_from_slice(&SCAN_RATE_HZ.to_le_bytes());
    out[16..20].copy_from_slice(&telemetry::last_proc_us().to_le_bytes());
    out[20] = wireless::battery();
    out[21] = TELEMETRY_UNAVAILABLE;
    out[22] = wireless::transport().code();
}

// === HID_KRO group (0x3x) ==================================================
//
// Rollover mode: boot 6-key rollover (the default) vs full N-key rollover. The
// toggle is held live in [`crate::usb`] and read by the keyboard report loop each
// scan; when NKRO is enabled the loop dual-sends the 6KRO boot report plus the
// NKRO bitmap (over USB and the radio alike), and when disabled it sends only the
// 6KRO report — byte-for-byte an unconfigured keyboard's 6KRO output. Like the RGB and BEHAVIOR
// groups the state is RAM-live and persisted as part of the CONFIG flash blob
// ([`CMD_CONFIG_SAVE`]); a reboot restores the saved value, or the firmware
// default (NKRO off, for boot-time compatibility) when nothing is saved.

/// HID_KRO `0x30` — get the rollover mode. No request payload; reply payload is
/// `[nkro_enabled]` (`1` = NKRO on, `0` = boot 6KRO).
pub const CMD_GET_KRO: u8 = 0x30;

/// HID_KRO `0x31` — set the rollover mode. Request payload `[0|1]` (`0` = boot
/// 6KRO, `1` = NKRO); any other value replies [`Status::BadArg`]. Applied live on
/// the next scan.
pub const CMD_SET_KRO: u8 = 0x31;

/// HID_KRO group handler. Reads and writes the live rollover toggle in
/// [`crate::usb`], so — like the KEYMAP, RGB and BEHAVIOR groups — it mutates
/// device state; the change is observed by the next keyboard report loop scan.
///
/// `req_payload` is the fixed 30-byte request region `req[2..32]`, so the single
/// argument byte `SET_KRO` reads is always present; `out` is the zeroed reply
/// payload `reply[3..32]`. An out-of-range argument maps to [`Status::BadArg`]; an
/// unrecognised operation is [`Status::BadCmd`].
fn hid_kro_dispatch(cmd: u8, req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_GET_KRO => {
            out[0] = crate::usb::nkro_enabled() as u8;
            Status::Ok
        }
        CMD_SET_KRO => match req_payload[0] {
            0 => {
                crate::usb::set_nkro(false);
                Status::Ok
            }
            1 => {
                crate::usb::set_nkro(true);
                Status::Ok
            }
            _ => Status::BadArg,
        },
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

// === CONFIG group (0x4x) ===================================================
//
// Persistence. Every group's settings — keymap, NKRO, RGB, SOCD, overrides,
// tap-dance, combos and macros — are edited in RAM by their own groups and lost
// on reboot; this group commits the *complete* live state to the reserved flash
// region as one CRC-protected blob (see [`crate::config`] / [`crate::flash`]) and
// restores it at boot. The split is deliberate: a live edit (e.g.
// `KEYMAP.SET_KEYCODE`) applies *live* and returns [`Status::Ok`] without
// touching flash, and the host calls [`CMD_CONFIG_SAVE`] when it wants those RAM
// edits to survive a power cycle. So a typical edit session is many live edits
// followed by one `SAVE`.

/// CONFIG `0x40` — persist the complete live state to flash.
///
/// No request payload. Snapshots every field of the [`crate::config`] blob layout
/// (the keymap and globals through the behaviour, timed, RGB-zone and feature-enable
/// blocks), erases and programs the reserved flash pages, and reads back to validate.
/// Replies [`Status::Ok`] on success or [`Status::Busy`] if the flash write or its
/// read-back verification failed (the live RAM state is unaffected either way).
pub const CMD_CONFIG_SAVE: u8 = 0x40;

/// CONFIG `0x41` — reset every group's live state to the firmware defaults.
///
/// No request payload. Restores the default keymap, boot 6KRO, the default RGB
/// state and empty behaviour/macro tables (see [`config::reset_to_defaults`]);
/// the change is live on the next scan but RAM-only — the host calls
/// [`CMD_CONFIG_SAVE`] afterwards to make it persist. Always [`Status::Ok`].
pub const CMD_CONFIG_LOAD_DEFAULTS: u8 = 0x41;

/// CONFIG `0x42` — describe the persistence region and stored blob.
///
/// No request payload; the reply payload layout is documented on
/// [`pack_storage_info`]. Always [`Status::Ok`].
pub const CMD_CONFIG_GET_STORAGE_INFO: u8 = 0x42;

/// CONFIG `0x43` — read the matrix debounce configuration.
///
/// No request payload; reply payload `[algorithm, interval]` — the active
/// [`matrix::DebounceAlgorithm`] code and the deferred-edge interval in consecutive
/// scans. Always [`Status::Ok`].
pub const CMD_CONFIG_GET_DEBOUNCE: u8 = 0x43;

/// CONFIG `0x44` — set the matrix debounce configuration.
///
/// Request payload `[algorithm, interval]`: the algorithm code (`0` =
/// symmetric-defer, `1` = asymmetric-eager-on-press) and the interval in consecutive
/// scans (`>= 1`). An unknown algorithm or a zero interval replies
/// [`Status::BadArg`]; otherwise the change is live on the next scan and
/// [`Status::Ok`]. RAM-only until a [`CMD_CONFIG_SAVE`] persists it.
pub const CMD_CONFIG_SET_DEBOUNCE: u8 = 0x44;

/// CONFIG `0x45` — read the runtime tunables (the timed engine's auto-shift, leader
/// and mod-tap / layer-tap settings, [`crate::timed`]).
///
/// No request payload; reply payload `[auto_shift_enabled(1), auto_shift_timeout(2
/// LE), leader_timeout(2 LE), tap_hold_term(2 LE), tap_hold_flags(1), quick_tap_term(2
/// LE)]` — the auto-shift enable flag and hold timeout in ms, the leader inter-key
/// timeout in ms, the mod-tap / layer-tap decision term in ms, the tap-hold flags byte
/// ([`TapHoldTuning::flags_byte`](crate::timed::TapHoldTuning::flags_byte): bit 0
/// permissive-hold, bit 1 hold-on-other-key-press, bit 2 retro-tapping, bit 3
/// chordal-hold) and the quick-tap window in ms (`0` = off). Always [`Status::Ok`].
pub const CMD_CONFIG_GET_TUNING: u8 = 0x45;

/// CONFIG `0x46` — set the runtime tunables.
///
/// Request payload mirrors [`CMD_CONFIG_GET_TUNING`]: `[auto_shift_enabled(1),
/// auto_shift_timeout(2 LE), leader_timeout(2 LE), tap_hold_term(2 LE),
/// tap_hold_flags(1), quick_tap_term(2 LE)]`. A zero auto-shift, leader or tap-hold
/// term replies [`Status::BadArg`] (a zero quick-tap window is valid and disables
/// quick-tap); otherwise the change is live on the next scan and [`Status::Ok`].
/// RAM-only until a [`CMD_CONFIG_SAVE`] persists it.
pub const CMD_CONFIG_SET_TUNING: u8 = 0x46;

/// Total length of the packed [`CMD_CONFIG_GET_STORAGE_INFO`] payload (see
/// [`pack_storage_info`]).
const STORAGE_INFO_LEN: usize = 11;
/// Length of the [`CMD_CONFIG_GET_TUNING`] reply payload: the auto-shift enable flag,
/// the auto-shift / leader / tap-hold timeouts (`u16` each), the tap-hold flags byte
/// and the quick-tap window.
const TUNING_LEN: usize = 10;

// Compile-time guarantee that the storage-info reply fits one reply payload
// (reply[3..32] = 29 bytes), like the INFO and TELEMETRY group checks.
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= STORAGE_INFO_LEN,
    "storage-info payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= TUNING_LEN,
    "tuning payload must fit one reply"
);

/// CONFIG group handler. Bridges kcp to the flash persistence layer in
/// [`crate::config`]: it can write flash ([`CMD_CONFIG_SAVE`]) and reset the live
/// RAM state ([`CMD_CONFIG_LOAD_DEFAULTS`]), so unlike the read-only groups it
/// legitimately changes device state.
///
/// `req_payload` is the fixed 30-byte request region `req[2..32]`, so the two
/// argument bytes `SET_DEBOUNCE` reads are always present; `out` is the zeroed reply
/// payload `reply[3..32]`. An unrecognised operation is [`Status::BadCmd`].
fn config_dispatch(cmd: u8, req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_CONFIG_SAVE => match config::save() {
            Ok(()) => Status::Ok,
            Err(()) => Status::Busy,
        },
        CMD_CONFIG_LOAD_DEFAULTS => {
            config::reset_to_defaults();
            Status::Ok
        }
        CMD_CONFIG_GET_STORAGE_INFO => {
            pack_storage_info(out);
            Status::Ok
        }
        CMD_CONFIG_GET_DEBOUNCE => {
            out[0] = matrix::algorithm() as u8;
            out[1] = matrix::interval();
            Status::Ok
        }
        CMD_CONFIG_SET_DEBOUNCE => {
            if matrix::set_debounce(req_payload[0], req_payload[1]) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        CMD_CONFIG_GET_TUNING => {
            let (as_on, as_timeout) = timed::autoshift_get();
            out[0] = as_on as u8;
            out[1..3].copy_from_slice(&as_timeout.to_le_bytes());
            out[3..5].copy_from_slice(&timed::leader_timeout_ms().to_le_bytes());
            let th = timed::taphold_get();
            out[5..7].copy_from_slice(&th.term_ms.to_le_bytes());
            out[7] = th.flags_byte();
            out[8..10].copy_from_slice(&th.quick_tap_term_ms.to_le_bytes());
            Status::Ok
        }
        CMD_CONFIG_SET_TUNING => {
            let as_timeout = u16::from_le_bytes([req_payload[1], req_payload[2]]);
            let leader_timeout = u16::from_le_bytes([req_payload[3], req_payload[4]]);
            let th_term = u16::from_le_bytes([req_payload[5], req_payload[6]]);
            let quick_tap = u16::from_le_bytes([req_payload[8], req_payload[9]]);
            // A zero auto-shift, leader or tap-hold term would fire instantly (or
            // never), so reject it like `SET_DEBOUNCE` rejects a zero interval. A zero
            // quick-tap window is valid — it disables quick-tap.
            if as_timeout == 0 || leader_timeout == 0 || th_term == 0 {
                return Status::BadArg;
            }
            timed::autoshift_set(req_payload[0] != 0, as_timeout);
            timed::leader_set_timeout(leader_timeout);
            timed::taphold_set(timed::TapHoldTuning::from_parts(
                th_term,
                req_payload[7],
                quick_tap,
            ));
            Status::Ok
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

/// Pack the storage descriptor for [`CMD_CONFIG_GET_STORAGE_INFO`] into the reply
/// payload. Every multi-byte field is little-endian. Layout, with
/// payload-relative byte offsets:
///
/// | bytes  | field                                              |
/// |--------|----------------------------------------------------|
/// | `0..4` | reserved-region base address (`u32`)               |
/// | `4..8` | reserved-region size, bytes (`u32`)                |
/// | `8..10`| stored blob format version (`u16`; 0 if none)      |
/// | `10`   | valid flag (`1` = a valid blob is stored, else `0`)|
///
/// Total [`STORAGE_INFO_LEN`] (11) bytes, within the 29-byte reply payload, which
/// the module-level `const` assertion guarantees. The values are sampled from
/// [`config::storage_info`] at call time (it reads the region back), so the
/// `valid`/`version` fields reflect what is actually in flash now.
fn pack_storage_info(out: &mut [u8]) {
    let info = config::storage_info();
    out[0..4].copy_from_slice(&info.base.to_le_bytes());
    out[4..8].copy_from_slice(&info.size.to_le_bytes());
    out[8..10].copy_from_slice(&info.version.to_le_bytes());
    out[10] = info.valid as u8;
}

// === MACRO group (0x5x) ====================================================
//
// Dynamic macros: a `MACRO(n)` keymap key (or the PLAY op) replays a recorded
// event sequence held in the RAM table in [`crate::timed`]. A macro is larger
// than one frame, so it is written one step per report — that *is* the chunking,
// keyed by `(macro, step)` — exactly fitting a 32-byte frame. A host can also
// upload steps directly (SET_STEP) or have the device record them on-board:
// RECORD_START captures live key edges into a slot until RECORD_STOP (QMK-style
// dynamic-macro record), with no keycode binding required. Each write mutates the
// shared timed-engine state and takes effect immediately, like the KEYMAP and
// BEHAVIOR groups; the table is RAM-live and persisted as part of the CONFIG
// flash blob ([`CMD_CONFIG_SAVE`]).

/// MACRO `0x50` — get the macro table capacities and the used-slot bitmap. No
/// request payload; reply `[MAX_MACRO, MAX_MACRO_STEPS, used(4 LE)]` where bit `i`
/// of `used` is set when macro `i` has at least one step.
pub const CMD_MACRO_INFO: u8 = 0x50;

/// MACRO `0x51` — set one step of a macro (the per-step chunked upload). Request
/// payload `[macro, step, kc_lo, kc_hi, down, delay_lo, delay_hi]`: the destination
/// `macro`/`step` indices, the step's keycode (little-endian), a `down` flag
/// (`1` = press, `0` = release) and the post-step delay in ms (little-endian
/// `u16`). The macro's length grows to cover `step`. An out-of-range `macro`/`step`
/// replies [`Status::BadArg`]. Applied live.
pub const CMD_MACRO_SET_STEP: u8 = 0x51;

/// MACRO `0x52` — read one step of a macro. Request payload `[macro, step]`; reply
/// `[present, kc_lo, kc_hi, down, delay_lo, delay_hi, len]` (`present` = 1 when
/// `step < len`, else 0) where `len` is the macro's active step count. An
/// out-of-range `macro`/`step` replies [`Status::BadArg`].
pub const CMD_MACRO_GET_STEP: u8 = 0x52;

/// MACRO `0x53` — clear a macro (length to zero). Request payload `[macro]`;
/// clears that slot, or all when `macro` is [`crate::behavior::CLEAR_ALL`]. An
/// out-of-range index (other than the sentinel) replies [`Status::BadArg`].
pub const CMD_MACRO_CLEAR: u8 = 0x53;

/// MACRO `0x54` — play a macro now (without a keymap binding). Request payload
/// `[macro]`; begins playback on the next scan. An out-of-range or empty `macro`
/// replies [`Status::BadArg`].
pub const CMD_MACRO_PLAY: u8 = 0x54;

/// MACRO `0x55` — start on-board recording into a macro slot. Request payload
/// `[macro]`; clears the slot and captures subsequent key press/release edges as
/// steps (QMK-style dynamic-macro record), with their inter-event timing, until
/// [`CMD_MACRO_RECORD_STOP`]. An out-of-range `macro` replies [`Status::BadArg`].
/// The keys still type live while recording. Applied live (RAM, persisted by the
/// CONFIG group like any macro edit).
pub const CMD_MACRO_RECORD_START: u8 = 0x55;

/// MACRO `0x56` — stop on-board recording. No request payload; ends any recording
/// started by [`CMD_MACRO_RECORD_START`] and always replies [`Status::Ok`] (a
/// stop with no recording in progress is a no-op success).
pub const CMD_MACRO_RECORD_STOP: u8 = 0x56;

/// Length of the [`CMD_MACRO_INFO`] reply payload.
const MACRO_INFO_LEN: usize = 6;
/// Length of the [`CMD_MACRO_GET_STEP`] reply payload.
const MACRO_GET_STEP_LEN: usize = 7;

const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= MACRO_INFO_LEN,
    "macro-info payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= MACRO_GET_STEP_LEN,
    "macro-get-step payload must fit one reply"
);

// === RGB group (0x6x) ======================================================
//
// Live control of the WS2812 lighting in [`crate::rgb`]: the base effect (mode,
// colour, brightness, on/off, speed), the status-indicator overlay, the zone table
// (`0x68..0x6B`, `0x6D` — each zone linked to the base effect, independent, disabled,
// resized or synced to another zone) and a host direct-streaming scaffold (`0x6C`).
// `0x60..0x67` drive the base effect and the status-indicator overlay, `0x68..0x6D`
// the zone table and the direct-stream scaffold. Each write is applied immediately to
// the shared RGB state the render task reads, so — like the KEYMAP group — this
// handler changes device state.
// The state is RAM-live and persisted as part of the CONFIG flash blob
// ([`CMD_CONFIG_SAVE`]). Every multi-report concern is avoided: the largest reply
// (`LIST_MODES`/`GET_STATE`/`GET_ZONE`) fits one report.

/// RGB `0x60` — set the effect mode. Request payload `[mode_id]`; an id outside
/// `0..`[`rgb::MODE_COUNT`] replies [`Status::BadArg`]. Applied live.
pub const CMD_RGB_SET_MODE: u8 = 0x60;

/// RGB `0x61` — set the effect colour. Request payload `[h, s, v]` (each
/// `0..=255`); always valid. Applied live.
pub const CMD_RGB_SET_HSV: u8 = 0x61;

/// RGB `0x62` — set the master brightness. Request payload `[val]` (`0..=255`);
/// always valid. Applied live.
pub const CMD_RGB_SET_BRIGHTNESS: u8 = 0x62;

/// RGB `0x63` — enable or disable RGB. Request payload `[0|1]`; any other value
/// replies [`Status::BadArg`]. Disabling cuts the LED rail.
pub const CMD_RGB_SET_ENABLED: u8 = 0x63;

/// RGB `0x64` — get the live RGB state. No request payload; the reply payload
/// layout is documented on [`pack_rgb_state`].
pub const CMD_RGB_GET_STATE: u8 = 0x64;

/// RGB `0x65` — list the available effect modes. Request payload `[start]` is the offset
/// into [`rgb::MODE_IDS`] to page from (`0` for the first page); the reply payload is
/// `[total, page_len, id_start, …]`, so the host pages by start offset until it has all
/// `total` ids (the set no longer fits one reply).
pub const CMD_RGB_LIST_MODES: u8 = 0x65;

/// RGB `0x66` — set the animation speed. Request payload `[speed]` (`0..=255`);
/// always valid. Applied live.
pub const CMD_RGB_SET_SPEED: u8 = 0x66;

/// RGB `0x67` — enable or disable the status-indicator overlay. Request payload
/// `[0|1]`; any other value replies [`Status::BadArg`]. Applied live and persisted
/// in the config blob (schema v7), so the choice survives a reboot.
pub const CMD_RGB_SET_INDICATORS: u8 = 0x67;

/// RGB `0x68` — get the zone-table summary. No request payload; reply payload is
/// `[zone_count, zone_cap]` — the number of zones the GUI lists ([`rgb::ZONE_COUNT`])
/// and the table capacity ([`rgb::ZONE_CAP`], with `0..zone_cap` addressable by
/// [`CMD_RGB_GET_ZONE`] / [`CMD_RGB_SET_ZONE`]).
pub const CMD_RGB_GET_ZONES: u8 = 0x68;

/// RGB `0x69` — get one zone's state. Request payload `[id]`; the reply payload
/// layout is documented on [`pack_zone`]. An `id >= `[`rgb::ZONE_CAP`] replies
/// [`Status::BadArg`].
pub const CMD_RGB_GET_ZONE: u8 = 0x69;

/// RGB `0x6A` — set one zone's effect params. Request payload
/// `[id, flags, mode, hue, sat, val, brightness, speed]`: `flags` is bit 0 ENABLED
/// (clear blanks the zone) | bit 1 LINKED (set mirrors the base effect's pixels in
/// the zone range, clear runs the zone's own `mode`), `mode` an effect id (outside
/// `0..`[`rgb::MODE_COUNT`] replies [`Status::BadArg`]), the rest `0..=255`. An
/// `id >= `[`rgb::ZONE_CAP`] also replies [`Status::BadArg`]. The zone's LED range is
/// set separately ([`CMD_RGB_SET_ZONE_RANGE`]). Applied live.
pub const CMD_RGB_SET_ZONE: u8 = 0x6A;

/// RGB `0x6B` — set one zone's LED range. Request payload
/// `[id, start_lo, start_hi, count_lo, count_hi]`: the zone's half-open chain range
/// `start..start+count` (little-endian `u16`s). An `id >= `[`rgb::ZONE_CAP`], or a
/// range past the chain (`start + count > `[`rgb::LED_COUNT`]), replies
/// [`Status::BadArg`]. Applied live.
pub const CMD_RGB_SET_ZONE_RANGE: u8 = 0x6B;

/// RGB `0x6C` — stream a host-rendered frame chunk (the OpenRGB/SignalRGB "Direct
/// mode" scaffold). Request payload `[offset_lo, offset_hi, len, rgb[len*3]]`: write
/// `len` `r, g, b` triples into the LED buffer at chain index `offset`. The firmware
/// then shows the host buffer verbatim, bypassing the base+zone effects, until the
/// stream goes idle (~1 s) and a watchdog reverts to the zone effects. A chunk that
/// overruns the request payload or the chain replies [`Status::BadArg`]. Applied
/// live; the host streaming engine is deferred to a future release.
pub const CMD_RGB_DIRECT: u8 = 0x6C;

/// RGB `0x6D` — set one zone's sync source. Request payload `[id, target]`: zone `id`
/// mirrors zone `target`'s effect settings (enabled/linked, mode, colour, brightness,
/// speed) live in its own LED range, or `target == `[`rgb::ZONE_SYNC_NONE`] (`0xFF`)
/// clears the link. An `id >= `[`rgb::ZONE_CAP`], a bad/self `target`, or a link that
/// would close a sync cycle replies [`Status::BadArg`]. Applied live.
pub const CMD_RGB_SET_ZONE_SYNC: u8 = 0x6D;

/// Total length of the packed [`CMD_RGB_GET_STATE`] payload (see
/// [`pack_rgb_state`]).
const RGB_STATE_LEN: usize = 10;

/// Total length of the packed [`CMD_RGB_GET_ZONE`] payload (see [`pack_zone`]).
const RGB_ZONE_LEN: usize = 13;

/// Mode ids carried per [`CMD_RGB_LIST_MODES`] reply page — the ids that fit after the
/// 2-byte `[total, page_len]` header. The effect set ([`rgb::MODE_IDS`]) outgrew a single
/// 29-byte reply, so the host pages through it by a start offset (like the keymap/macro
/// getters), reassembling the full list across replies.
const RGB_MODE_PAGE: usize = MSG_LEN - REPLY_PAYLOAD_IDX - 2;
/// Length of a full [`CMD_RGB_LIST_MODES`] reply page: the `[total, page_len]` header plus
/// up to [`RGB_MODE_PAGE`] mode ids.
const RGB_MODE_LIST_LEN: usize = 2 + RGB_MODE_PAGE;

// Compile-time guarantees that the variable replies fit one reply payload
// (reply[3..32] = 29 bytes), like the other groups' checks.
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= RGB_STATE_LEN,
    "rgb-state payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= RGB_ZONE_LEN,
    "rgb zone payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= RGB_MODE_LIST_LEN,
    "rgb mode list must fit one reply"
);

/// RGB group handler. Writes the shared live RGB state in [`crate::rgb`], so it
/// legitimately mutates device state.
///
/// `req_payload` is the fixed 30-byte request region `req[2..32]`, so the few
/// leading argument bytes each operation reads are always present; `out` is the
/// zeroed reply payload `reply[3..32]`. Out-of-range arguments map to
/// [`Status::BadArg`]; an unrecognised operation is [`Status::BadCmd`].
fn rgb_dispatch(cmd: u8, req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_RGB_SET_MODE => {
            if rgb::set_mode(req_payload[0]) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        CMD_RGB_SET_HSV => {
            rgb::set_hsv(req_payload[0], req_payload[1], req_payload[2]);
            Status::Ok
        }
        CMD_RGB_SET_BRIGHTNESS => {
            rgb::set_brightness(req_payload[0]);
            Status::Ok
        }
        CMD_RGB_SET_SPEED => {
            rgb::set_speed(req_payload[0]);
            Status::Ok
        }
        CMD_RGB_SET_ENABLED => match req_payload[0] {
            0 => {
                rgb::set_enabled(false);
                Status::Ok
            }
            1 => {
                rgb::set_enabled(true);
                Status::Ok
            }
            _ => Status::BadArg,
        },
        CMD_RGB_SET_INDICATORS => match req_payload[0] {
            0 => {
                rgb::set_indicators(false);
                Status::Ok
            }
            1 => {
                rgb::set_indicators(true);
                Status::Ok
            }
            _ => Status::BadArg,
        },
        CMD_RGB_GET_STATE => {
            pack_rgb_state(out);
            Status::Ok
        }
        CMD_RGB_LIST_MODES => {
            let start = (req_payload[0] as usize).min(rgb::MODE_IDS.len());
            let n = (rgb::MODE_IDS.len() - start).min(RGB_MODE_PAGE);
            out[0] = rgb::MODE_COUNT;
            out[1] = n as u8;
            out[2..2 + n].copy_from_slice(&rgb::MODE_IDS[start..start + n]);
            Status::Ok
        }
        CMD_RGB_GET_ZONES => {
            out[0] = rgb::ZONE_COUNT as u8;
            out[1] = rgb::ZONE_CAP as u8;
            Status::Ok
        }
        CMD_RGB_GET_ZONE => match rgb::zone(req_payload[0] as usize) {
            Some(z) => {
                pack_zone(req_payload[0], &z, out);
                Status::Ok
            }
            None => Status::BadArg,
        },
        CMD_RGB_SET_ZONE => {
            // [id, flags, mode, hue, sat, val, brightness, speed]
            if rgb::set_zone(
                req_payload[0] as usize,
                req_payload[1],
                req_payload[2],
                (req_payload[3], req_payload[4], req_payload[5]),
                req_payload[6],
                req_payload[7],
            ) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        CMD_RGB_SET_ZONE_RANGE => {
            // [id, start:u16le, count:u16le]. The range must fit the chain *and* stay
            // disjoint from the other lit zones (overlap check first, short-circuiting
            // the bounds-only setter).
            let id = req_payload[0] as usize;
            let start = u16::from_le_bytes([req_payload[1], req_payload[2]]);
            let count = u16::from_le_bytes([req_payload[3], req_payload[4]]);
            if !rgb::zone_range_overlaps(id, start, count) && rgb::set_zone_range(id, start, count) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        CMD_RGB_SET_ZONE_SYNC => {
            // [id, target]; target 0xFF clears. `set_zone_sync` validates the id/target
            // and rejects a sync cycle.
            if rgb::set_zone_sync(req_payload[0] as usize, req_payload[1]) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        CMD_RGB_DIRECT => {
            // [offset:u16le, len, rgb[len*3]]. The handler owns the framing bound (the
            // chunk must fit the 30-byte request payload); `direct_write` owns the
            // chain bound (offset + len must fit the LED chain).
            let offset = u16::from_le_bytes([req_payload[0], req_payload[1]]) as usize;
            let len = req_payload[2] as usize;
            let end = 3 + len * 3;
            if end <= req_payload.len() && rgb::direct_write(offset, &req_payload[3..end]) {
                Status::Ok
            } else {
                Status::BadArg
            }
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

/// Pack the live RGB state for [`CMD_RGB_GET_STATE`] into the reply payload.
/// Layout, with payload-relative byte offsets:
///
/// | bytes  | field                                              |
/// |--------|----------------------------------------------------|
/// | `0`    | effect mode ([`rgb::mode`])                        |
/// | `1`    | hue                                                |
/// | `2`    | saturation                                         |
/// | `3`    | value                                              |
/// | `4`    | master brightness ([`rgb::brightness`])           |
/// | `5`    | enabled (`1`/`0`)                                  |
/// | `6..8` | LED count ([`rgb::LED_COUNT`]) as a `u16`          |
/// | `8`    | animation speed ([`rgb::speed`])                  |
/// | `9`    | status indicators enabled (`1`/`0`)               |
///
/// Total [`RGB_STATE_LEN`] (10) bytes, within the 29-byte reply payload, which
/// the module-level `const` assertion guarantees. The values are sampled from
/// the shared RGB state at call time.
fn pack_rgb_state(out: &mut [u8]) {
    let (h, s, v) = rgb::hsv();
    out[0] = rgb::mode();
    out[1] = h;
    out[2] = s;
    out[3] = v;
    out[4] = rgb::brightness();
    out[5] = rgb::enabled() as u8;
    out[6..8].copy_from_slice(&(rgb::LED_COUNT as u16).to_le_bytes());
    out[8] = rgb::speed();
    out[9] = rgb::indicators_enabled() as u8;
}

/// Pack zone `id`'s state for [`CMD_RGB_GET_ZONE`] into the reply payload. Layout,
/// with payload-relative byte offsets:
///
/// | bytes    | field                                              |
/// |----------|----------------------------------------------------|
/// | `0`      | zone id (echoed)                                   |
/// | `1`      | flags (bit 0 ENABLED, bit 1 LINKED)                |
/// | `2`      | independent effect mode ([`rgb::ZoneState`])       |
/// | `3`      | independent hue                                    |
/// | `4`      | independent saturation                             |
/// | `5`      | independent value                                  |
/// | `6`      | independent master brightness                      |
/// | `7`      | independent animation speed                        |
/// | `8..10`  | range start (`u16` LE)                             |
/// | `10..12` | range LED count (`u16` LE)                         |
/// | `12`     | sync source: `0` = not synced, else zone id + 1    |
///
/// Total [`RGB_ZONE_LEN`] (13) bytes — within the 29-byte reply payload, which the
/// module-level `const` assertion guarantees. Sampled from the live zone table; the
/// bytes `1..8` are exactly what [`CMD_RGB_SET_ZONE`] accepts, so a host round-trips
/// them unchanged. The sync byte uses the biased [`rgb::sync_to_wire`] encoding (`0`
/// = not synced) so a zeroed slot decodes to not-synced; it is set separately
/// ([`CMD_RGB_SET_ZONE_SYNC`]).
fn pack_zone(id: u8, z: &rgb::ZoneState, out: &mut [u8]) {
    out[0] = id;
    out[1] = z.flags;
    out[2] = z.mode;
    out[3] = z.h;
    out[4] = z.s;
    out[5] = z.v;
    out[6] = z.bright;
    out[7] = z.speed;
    out[8..10].copy_from_slice(&z.start.to_le_bytes());
    out[10..12].copy_from_slice(&z.count.to_le_bytes());
    out[12] = rgb::sync_to_wire(z.sync_to);
}

// === BEHAVIOR group (0x7x) =================================================
//
// Stateless input behaviours: SOCD cleanup and key overrides, held in the RAM
// tables in [`crate::behavior`] and applied live while [`keymap::compute_report`]
// builds each report. Each write here mutates that shared state — like the KEYMAP
// and RGB groups — and takes effect on the next scan. The tables are RAM-live and
// persisted as part of the CONFIG flash blob ([`CMD_CONFIG_SAVE`]). Keycodes cross
// the wire as little-endian `u16`s, matching the KEYMAP group.

/// BEHAVIOR `0x70` — configure a SOCD pair.
///
/// Request payload `[index, a_lo, a_hi, b_lo, b_hi, mode]`: slot `index`, the two
/// opposing keycodes (little-endian), and a [`crate::behavior::SocdMode`] byte
/// (`0` = LastWins, `1` = Neutral, `2` = FirstWins). An out-of-range `index` or
/// an unassigned `mode` replies [`Status::BadArg`]; the pair's press-order state
/// is reset. Applied live.
pub const CMD_SOCD_SET: u8 = 0x70;

/// BEHAVIOR `0x71` — clear a SOCD pair.
///
/// Request payload `[index]`; clears that slot, or the whole table when `index`
/// is [`crate::behavior::CLEAR_ALL`] (`0xFF`). An out-of-range `index` (other than the
/// sentinel) replies [`Status::BadArg`].
pub const CMD_SOCD_CLEAR: u8 = 0x71;

/// BEHAVIOR `0x72` — read a SOCD pair.
///
/// Request payload `[index]`; reply payload is `[present, a_lo, a_hi, b_lo, b_hi,
/// mode]` (`present` = 1 with the pair, or 0 for an empty slot). An out-of-range
/// `index` replies [`Status::BadArg`].
pub const CMD_SOCD_GET: u8 = 0x72;

/// BEHAVIOR `0x73` — configure a key override.
///
/// Request payload `[index, trig_lo, trig_hi, trig_mods, repl_lo, repl_hi,
/// repl_mods, layer_lo, layer_hi, enabled]`: slot `index`, the trigger keycode
/// and the modifier byte it must match exactly, the replacement keycode and its
/// modifier byte, the active-layer mask (little-endian `u16`) and an enabled
/// flag. An out-of-range `index` replies [`Status::BadArg`]. Applied live.
pub const CMD_OVERRIDE_SET: u8 = 0x73;

/// BEHAVIOR `0x74` — clear a key override.
///
/// Request payload `[index]`; clears that slot, or the whole table when `index`
/// is [`crate::behavior::CLEAR_ALL`] (`0xFF`). An out-of-range `index` (other than the
/// sentinel) replies [`Status::BadArg`].
pub const CMD_OVERRIDE_CLEAR: u8 = 0x74;

/// BEHAVIOR `0x75` — read a key override.
///
/// Request payload `[index]`; reply payload is `[present, trig_lo, trig_hi,
/// trig_mods, repl_lo, repl_hi, repl_mods, layer_lo, layer_hi, enabled]`
/// (`present` = 1 with the override, or 0 for an empty slot). An out-of-range
/// `index` replies [`Status::BadArg`].
pub const CMD_OVERRIDE_GET: u8 = 0x75;

/// BEHAVIOR `0x76` — get the behaviour table capacities.
///
/// No request payload; reply payload is `[MAX_SOCD, MAX_OVERRIDES]`
/// ([`crate::behavior::MAX_SOCD`], [`crate::behavior::MAX_OVERRIDES`]).
pub const CMD_BEHAVIOR_INFO: u8 = 0x76;

/// BEHAVIOR `0x77` — configure a tap-dance entry (the timed engine,
/// [`crate::timed`]). Request payload `[index, tap_lo, tap_hi, hold_lo, hold_hi,
/// dbl_lo, dbl_hi, term_lo, term_hi]`: slot `index`, the tap / hold / double-tap
/// keycodes (little-endian; a double keycode of `0` = `NONE` falls back to the
/// tap) and the decision window `term` in ms (little-endian `u16`). An
/// out-of-range `index` replies [`Status::BadArg`]. Applied live (RAM).
pub const CMD_TAPDANCE_SET: u8 = 0x77;

/// BEHAVIOR `0x78` — read a tap-dance entry. Request payload `[index]`; reply
/// `[present, tap_lo, tap_hi, hold_lo, hold_hi, dbl_lo, dbl_hi, term_lo, term_hi]`
/// (`present` = 1 with the entry, 0 for an empty slot). Out-of-range → BadArg.
pub const CMD_TAPDANCE_GET: u8 = 0x78;

/// BEHAVIOR `0x79` — clear a tap-dance entry. Request payload `[index]`; clears
/// that slot, or all when `index` is [`crate::behavior::CLEAR_ALL`]. Out-of-range (other
/// than the sentinel) → BadArg.
pub const CMD_TAPDANCE_CLEAR: u8 = 0x79;

/// BEHAVIOR `0x7A` — configure a combo (the timed engine). Request payload
/// `[index, len, k0_lo, k0_hi, k1_lo, k1_hi, k2_lo, k2_hi, k3_lo, k3_hi, act_lo,
/// act_hi, term_lo, term_hi, flags]`: slot `index`, key count `len`
/// (`2..=MAX_COMBO_KEYS`), up to four member keycodes (little-endian; only the
/// first `len` are used), the action keycode, the window `term` in ms and the
/// per-combo `flags` byte ([`timed::ComboCfg`] `FLAG_*`: must-hold / must-tap /
/// in-order). An out-of-range `index`, a bad `len`, a duplicate member keycode, an
/// unknown flag bit or the must-hold + must-tap pair replies [`Status::BadArg`].
/// Applied live.
pub const CMD_COMBO_SET: u8 = 0x7A;

/// BEHAVIOR `0x7B` — read a combo. Request payload `[index]`; reply `[present,
/// len, k0_lo, k0_hi, k1_lo, k1_hi, k2_lo, k2_hi, k3_lo, k3_hi, act_lo, act_hi,
/// term_lo, term_hi, flags]`. Out-of-range → BadArg.
pub const CMD_COMBO_GET: u8 = 0x7B;

/// BEHAVIOR `0x7C` — clear a combo. Request payload `[index]`; clears that slot,
/// or all when `index` is [`crate::behavior::CLEAR_ALL`]. Out-of-range → BadArg.
pub const CMD_COMBO_CLEAR: u8 = 0x7C;

/// BEHAVIOR `0x7D` — get the timed-engine table capacities. No request payload;
/// reply `[MAX_TAP_DANCE, MAX_COMBO, MAX_COMBO_KEYS, MAX_MACRO, MAX_MACRO_STEPS,
/// MAX_LEADER, MAX_LEADER_SEQ]`.
pub const CMD_TIMED_INFO: u8 = 0x7D;

/// BEHAVIOR `0x7E` — configure a leader-sequence entry (the timed engine,
/// [`crate::timed`]). Request payload `[index, len, s0_lo, s0_hi, … s4_lo, s4_hi,
/// act_lo, act_hi]`: slot `index`, the sequence length `len` (`0..=MAX_LEADER_SEQ`;
/// `0` clears the slot), up to [`timed::MAX_LEADER_SEQ`] sequence keycodes
/// (little-endian; only the first `len` are used) and the action keycode. An
/// out-of-range `index` or a `len` above the cap replies [`Status::BadArg`]. Applied
/// live (RAM).
pub const CMD_LEADER_SET: u8 = 0x7E;

/// BEHAVIOR `0x7F` — read a leader-sequence entry. Request payload `[index]`; reply
/// `[len, s0_lo, s0_hi, … s4_lo, s4_hi, act_lo, act_hi]` (`len == 0` for an empty
/// slot). An out-of-range `index` replies [`Status::BadArg`].
pub const CMD_LEADER_GET: u8 = 0x7F;

/// Length of the [`CMD_SOCD_GET`] reply payload (see its constant).
const SOCD_GET_LEN: usize = 6;
/// Length of the [`CMD_OVERRIDE_GET`] reply payload (see its constant).
const OVERRIDE_GET_LEN: usize = 10;
/// Length of the [`CMD_TAPDANCE_GET`] reply payload.
const TAPDANCE_GET_LEN: usize = 9;
/// Length of the [`CMD_COMBO_GET`] reply payload (the 14-byte combo record plus the
/// per-combo flags byte).
const COMBO_GET_LEN: usize = 15;
/// Length of the [`CMD_TIMED_INFO`] reply payload: the five table capacities plus
/// the two leader capacities.
const TIMED_INFO_LEN: usize = 7;
/// Length of the [`CMD_LEADER_GET`] reply payload: `len` + the sequence keycodes +
/// the action keycode.
const LEADER_GET_LEN: usize = 1 + timed::MAX_LEADER_SEQ * 2 + 2;

// Compile-time guarantees that both variable replies fit one reply payload
// (reply[3..32] = 29 bytes), like the other groups' checks.
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= SOCD_GET_LEN,
    "socd-get payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= OVERRIDE_GET_LEN,
    "override-get payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= TAPDANCE_GET_LEN,
    "tapdance-get payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= COMBO_GET_LEN,
    "combo-get payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= TIMED_INFO_LEN,
    "timed-info payload must fit one reply"
);
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= LEADER_GET_LEN,
    "leader-get payload must fit one reply"
);

// === WIRELESS group (0x8x) =================================================
//
// Wireless transport control: read the link / battery state and drive the
// connection state machine in [`crate::wireless`]. Each control op enqueues `md`
// frames — non-blocking, dropped when the radio TX queue is full, like the
// vendor `smsg_push` — so this handler keeps `handle`'s no-block contract while
// legitimately mutating device state, like the KEYMAP and RGB groups. It is
// reachable identically over USB and, via the 2.4G dongle bridge, over the
// radio (`kcp_radio_task`), so a host configures wireless the same way on either
// link.

/// WIRELESS `0x80` — get the link snapshot. No request payload; reply payload is
/// `[devs, state, battery, version]` (see [`pack_wireless_state`]).
pub const CMD_WLS_GET_STATE: u8 = 0x80;

/// WIRELESS `0x81` — select the output transport without a pairing reset.
/// Request payload `[devs]` (a [`wireless::Devs`] code); an unknown code replies
/// [`Status::BadArg`]. Reconnects to the existing bond.
pub const CMD_WLS_SET_MODE: u8 = 0x81;

/// WIRELESS `0x82` — (re)pair the current transport. No request payload; runs
/// the device's pairing sequence with `reset = true` (clear bond → advertise →
/// pair).
pub const CMD_WLS_PAIR: u8 = 0x82;

/// WIRELESS `0x83` — clear the active channel's bond (`DEVCTRL_CLEAN`). No
/// request payload.
pub const CMD_WLS_UNPAIR: u8 = 0x83;

/// WIRELESS `0x84` — set the radio idle-sleep policy. Request payload
/// `[enable_bt, enable_2g4]` (each `0` = disable, non-zero = enable).
pub const CMD_WLS_SET_SLEEP_POLICY: u8 = 0x84;

/// WIRELESS `0x85` — get the battery level and trigger a refresh. Reply payload
/// `[battery]` (the last reported level); also enqueues an `INQVOL` so the next
/// read reflects a fresh measurement.
pub const CMD_WLS_GET_BATTERY: u8 = 0x85;

/// Length of the [`CMD_WLS_GET_STATE`] reply payload (see [`pack_wireless_state`]).
const WIRELESS_STATE_LEN: usize = 4;

// Compile-time guarantee the state reply fits one reply payload (29 bytes), like
// the other groups' checks.
const _: () = assert!(
    MSG_LEN - REPLY_PAYLOAD_IDX >= WIRELESS_STATE_LEN,
    "wireless-state payload must fit one reply"
);

/// WIRELESS group handler. Drives the [`crate::wireless`] connection state
/// machine and reads its live link / battery state.
///
/// `req_payload` is the fixed 30-byte request region `req[2..32]`, so the leading
/// argument bytes each op reads are always present; `out` is the zeroed reply
/// payload `reply[3..32]`. An unknown transport code maps to [`Status::BadArg`];
/// an unrecognised operation is [`Status::BadCmd`].
fn wireless_dispatch(cmd: u8, req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_WLS_GET_STATE => {
            pack_wireless_state(out);
            Status::Ok
        }
        CMD_WLS_SET_MODE => match wireless::Devs::from_u8(req_payload[0]) {
            // Only wireless transports are host-settable. USB is the cable-auto-
            // selected top priority, not an output the host picks: forcing it while on
            // battery would route HID to a link with no host (dead keys), recoverable
            // only by a cable cycle. It is reached by plugging in, so reject it here.
            Some(devs) if devs.is_wireless() => {
                // Record the explicit wireless choice as the fallback preference so a
                // later USB unplug returns to it. The switch still applies immediately:
                // the supervisor overrides only on a USB plug/unplug edge, so the
                // chosen mode stands until then.
                wireless::set_preferred_wireless(devs);
                wireless::devs_change(devs, false);
                Status::Ok
            }
            _ => Status::BadArg,
        },
        CMD_WLS_PAIR => {
            // Pairing the current transport also makes it the preferred wireless (USB
            // is ignored), so a later unplug returns to the freshly paired channel.
            let devs = wireless::transport();
            wireless::set_preferred_wireless(devs);
            wireless::devs_change(devs, true);
            Status::Ok
        }
        CMD_WLS_UNPAIR => {
            wireless::unpair();
            Status::Ok
        }
        CMD_WLS_SET_SLEEP_POLICY => {
            wireless::set_sleep_policy(req_payload[0] != 0, req_payload[1] != 0);
            Status::Ok
        }
        CMD_WLS_GET_BATTERY => {
            out[0] = wireless::battery();
            wireless::inquire_battery();
            Status::Ok
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}

/// Pack the wireless link snapshot for [`CMD_WLS_GET_STATE`] into the reply
/// payload. Layout, with payload-relative byte offsets:
///
/// | bytes | field                                              |
/// |-------|----------------------------------------------------|
/// | `0`   | active transport ([`wireless::Devs`] code)         |
/// | `1`   | connection state (`MD_STATE_*`)                    |
/// | `2`   | battery percent (`md_info.bat`)                    |
/// | `3`   | radio firmware version                             |
///
/// Total [`WIRELESS_STATE_LEN`] (4) bytes, sampled from [`crate::wireless`] at
/// call time.
fn pack_wireless_state(out: &mut [u8]) {
    out[0] = wireless::transport().code();
    out[1] = wireless::connection_state();
    out[2] = wireless::battery();
    out[3] = wireless::radio_version();
}

// === TEXT group (0x9x) =====================================================
//
// Text-input behaviours, owned by the autocorrect registry feature
// ([`crate::features::autocorrect`]). Like the MACRO and BEHAVIOR groups it routes
// through `features::run_on_kcp`, so the operation codes are declared here (next to
// the other groups') while the handler lives in the feature. The shipped typo
// dictionary is compiled in, so there is no upload op; `0x92..=0x9F` are reserved
// for a future host-uploadable dictionary.

/// TEXT `0x90` — get the autocorrect state. No request payload; reply payload is
/// `[enabled, count_lo, count_hi]`: the enable flag and the compiled-in dictionary
/// entry count (little-endian `u16`). Always [`Status::Ok`]. Gated with the feature that
/// owns the group, so a build without it neither defines the opcode nor routes the group.
#[cfg(feature = "autocorrect")]
pub const CMD_TEXT_AUTOCORRECT_INFO: u8 = 0x90;

/// TEXT `0x91` — enable or disable autocorrect. Request payload `[0|1]`; any other value
/// replies [`Status::BadArg`]. An alias for `FEATURES.SET_ENABLED` on autocorrect's bit:
/// applied live and persisted in the config blob's feature-enable bitmap (schema v10), so the
/// choice survives a reboot.
#[cfg(feature = "autocorrect")]
pub const CMD_TEXT_AUTOCORRECT_SET: u8 = 0x91;

// === UNICODE group (0xAx) ==================================================
//
// Unicode input — the active OS input mode and the host-uploaded codepoint map.
// Owned entirely by the [`crate::features::unicode`] plugin (routed through
// `features::run_on_kcp`), so the wire format lives there; these opcodes are
// declared here, beside every other group's, as the single source the TS client
// and the drift check mirror. The map is RAM-only (no `config` persistence), so the
// host re-uploads it on connect.

/// UNICODE `0xA0` — get the Unicode-input state. Reply payload
/// `[active_mode, slot_count, mode_count]`.
#[cfg(feature = "unicode")]
pub const CMD_UNICODE_GET: u8 = 0xA0;
/// UNICODE `0xA1` — set the active OS input mode. Request `[mode]` (`0` = Linux/IBus,
/// `1` = macOS, `2` = Windows/WinCompose); an out-of-range mode replies [`Status::BadArg`].
#[cfg(feature = "unicode")]
pub const CMD_UNICODE_SET_MODE: u8 = 0xA1;
/// UNICODE `0xA2` — upload one codepoint slot. Request `[slot, cp(4, LE u32)]`; an
/// out-of-range slot replies [`Status::BadArg`]. A `0` codepoint clears the slot (it then
/// types nothing).
#[cfg(feature = "unicode")]
pub const CMD_UNICODE_SET_MAP: u8 = 0xA2;

// === FEATURES group (0xDx) =================================================
//
// Registry-owned runtime enable/disable. The dispatcher iterates
// [`crate::features::FEATURES`] (`features::features_dispatch`), so every registered
// feature is enumerated and toggled with no per-feature wiring. These opcodes are
// declared here, beside every other group's, as the single source the TS client and
// the drift check mirror. The enable bitmap is persisted in the config blob, so a SET
// is live but unsaved until `CONFIG.SAVE`.

/// FEATURES `0xD0` — enumerate the registered features. Request `[start]`; reply
/// `[count, page_len, {id, enabled, name_len, name_bytes}…]` packs as many records
/// from index `start` as fit one frame, so the host pages until it has all `count`.
pub const CMD_GET_FEATURES: u8 = 0xD0;
/// FEATURES `0xD1` — switch one feature on or off. Request `[id, 0|1]`; an unknown id,
/// an attempt to disable an always-on feature, or a non-boolean value replies
/// [`Status::BadArg`]. Applied live, persisted by the next `CONFIG.SAVE`.
pub const CMD_SET_FEATURE_ENABLED: u8 = 0xD1;

// === SYSTEM group (0xFx) ===================================================
//
// MCU- and USB-device-level control. The two *reset* ops (ENTER_DFU / REBOOT)
// reset the chip, so — unlike the other groups — they never produce a reply the
// host can read: [`handle`] resets before it returns, exactly as QMK's
// `RESET`/`QK_BOOT` does, and the GUI treats the USB disconnect as the
// acknowledgement. The USB-personality ops select the device's USB mode: SET_USB_MODE
// re-enumerates the device as MIDI / XInput / the normal composite (a change drops
// the USB pipe, so it is acknowledged by the disconnect like a reset), GET_USB_MODE
// reads the current mode back, and SET_DIGITIZER injects an absolute-pointer
// position for the HID digitizer (a host/test control). Factory reset and save-all
// remain the CONFIG group's job (LOAD_DEFAULTS / SAVE).

/// SYSTEM `0xF0` — reset into the `wb32-dfu` bootloader.
///
/// No request payload. Arms the magic word and resets via
/// [`crate::boot::bootloader_jump`]; the reset path then enters the ROM
/// bootloader (see [`crate::boot`]). The device resets before a reply is sent,
/// so the host receives none.
pub const CMD_SYSTEM_ENTER_DFU: u8 = 0xF0;

/// SYSTEM `0xF1` — reboot the firmware.
///
/// No request payload. A plain [`crate::boot::reboot`] (no magic), so the reset
/// path boots the firmware normally. The device resets before a reply is sent,
/// so the host receives none.
pub const CMD_SYSTEM_REBOOT: u8 = 0xF1;

/// SYSTEM `0xF2` — select the USB device personality. Request payload `[mode]`
/// (`0` = normal composite, `1` = MIDI, `2` = XInput). A known but *different*
/// mode re-enumerates the device (the keyboard detaches, rebuilds its descriptor
/// and re-attaches), which drops the USB pipe — so, like a reset, the disconnect is
/// the acknowledgement and the [`Status::Ok`] reply may not reach the host.
/// Re-selecting the current mode is a no-op that replies normally; an out-of-range
/// mode is [`Status::BadArg`]. See [`crate::usb::request_usb_mode`].
pub const CMD_SYSTEM_SET_USB_MODE: u8 = 0xF2;

/// SYSTEM `0xF3` — get the current USB device personality. No request payload;
/// reply payload `[mode]` (the [`crate::usb::UsbMode`] wire code). Lets the host
/// reflect the active mode after a re-enumeration (the kcp interface is present in
/// every mode, so this is always reachable).
pub const CMD_SYSTEM_GET_USB_MODE: u8 = 0xF3;

/// SYSTEM `0xF4` — set the HID digitizer's absolute pointer position (a host/test
/// control; the keymap owns no digitizer keycode). Request payload
/// `[flags, x_lo, x_hi, y_lo, y_hi]`: `flags` bit 0 is the tip switch (touching),
/// bit 1 is in-range, and X/Y are unsigned little-endian over `0..=32767`
/// ([`crate::digitizer::LOGICAL_MAX`], clamped). The contact is emitted on the
/// shared interface's digitizer report in the normal composite. Always
/// [`Status::Ok`]. See [`crate::digitizer::set`].
pub const CMD_SYSTEM_SET_DIGITIZER: u8 = 0xF4;

/// SYSTEM group handler. The two reset ops (ENTER_DFU / REBOOT) reset the MCU and
/// never return; the USB-personality ops reply normally (SET_USB_MODE may instead
/// be acknowledged by the re-enumeration disconnect when the mode changes). An
/// unrecognised operation is [`Status::BadCmd`].
fn system_dispatch(cmd: u8, req_payload: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        CMD_SYSTEM_ENTER_DFU => crate::boot::bootloader_jump(),
        CMD_SYSTEM_REBOOT => crate::boot::reboot(),
        CMD_SYSTEM_SET_USB_MODE => match crate::usb::request_usb_mode(req_payload[0]) {
            Some(_) => Status::Ok,
            None => Status::BadArg,
        },
        CMD_SYSTEM_GET_USB_MODE => {
            out[0] = crate::usb::usb_mode_code();
            Status::Ok
        }
        CMD_SYSTEM_SET_DIGITIZER => {
            let flags = req_payload[0];
            let x = u16::from_le_bytes([req_payload[1], req_payload[2]]);
            let y = u16::from_le_bytes([req_payload[3], req_payload[4]]);
            crate::digitizer::set(x, y, flags & 0x01 != 0, flags & 0x02 != 0);
            Status::Ok
        }
        // Known group, unrecognised operation code.
        _ => Status::BadCmd,
    }
}
