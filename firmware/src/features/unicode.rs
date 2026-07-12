// SPDX-License-Identifier: GPL-2.0-or-later
//! Unicode input — type any codepoint through the host OS's own input method.
//!
//! Three keycodes drive it (see [`crate::keycode`]): `UNICODE_MODE_CYCLE` cycles the
//! active OS mode, and `UM(0)..=UM(15)` each emit one slot of a host-uploaded codepoint
//! table. A `UM(n)` press ([`KeyAction::UnicodeMap`](crate::keycode::KeyAction::UnicodeMap),
//! fired on its press edge by [`crate::keymap::compute_report`]) builds the *active mode's*
//! key sequence for `map[n]` and plays it back over the following scans — like a mini
//! macro — so the 1 kHz scan is never blocked. There is no codepoint→key table in
//! firmware: the OS does the lookup; the firmware only types the right keystrokes.
//!
//! # The three OS senders
//!
//! Mirrors QMK's `unicode_input_start` / `register_hex32` / `unicode_input_finish`, the
//! reference implementation:
//!
//! * **Linux (IBus)** — tap Ctrl+Shift+U, type the codepoint's hex digits, then Space to
//!   commit. The de-facto IBus Unicode entry sequence.
//! * **macOS (Unicode Hex Input)** — hold Left Option, type the UTF-16 code unit(s) as
//!   exactly-four-hex-digit groups, release Option. A non-BMP codepoint is sent as its
//!   surrogate pair (two four-digit groups under the same held Option), which is how the
//!   OS layout assembles astral-plane characters.
//! * **Windows (WinCompose)** — tap the compose key (Right Alt), tap `U`, type the hex
//!   digits (a leading `0` first when the first digit is a-f), then Enter. Targets the
//!   WinCompose utility's `RAlt`, `U` Unicode shortcut.
//!
//! # Playback without blocking
//!
//! The sequence is a list of [`Frame`]s (a modifier byte + a basic usage). Each is
//! presented for [`FRAME_DWELL_SCANS`] scans so the ~10 ms host poll catches every press
//! and release; [`Feature::on_overlay`] merges the current frame into the report and
//! [`Feature::on_tick`] advances the player. A modifier carried in *every* frame of a run
//! (Option, for macOS) stays held across the whole run, then a trailing empty frame
//! releases it. The `active()` gate is one relaxed load, so an idle keyboard pays nothing.
//!
//! # State and persistence
//!
//! The codepoint table and the active mode live in RAM only — there is intentionally no
//! [`crate::config`] persistence and no schema field, so the host re-uploads the map (kcp
//! group `0xA`) on connect, exactly as a freshly-powered keyboard expects.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::Instant;

use crate::features::{Ctx, Feature, FeatureId};
use crate::kcp::{self, Status};
use crate::keymap::{self, Report};

/// Host-uploadable codepoint slots — one `UM(n)` keycode per slot.
pub const UNICODE_MAP_SLOTS: usize = 16;
/// Number of OS input modes the sender knows (Linux / macOS / Windows).
pub const UNICODE_MODE_COUNT: u8 = 3;

/// Linux (IBus) input mode.
const MODE_LINUX: u8 = 0;
/// macOS (Unicode Hex Input) input mode.
const MODE_MACOS: u8 = 1;
/// Windows (WinCompose) input mode.
const MODE_WINDOWS: u8 = 2;

/// Frames one codepoint sequence can need. The WinCompose path is the worst case: the RAlt
/// tap, the `U` tap, up to six hex-digit taps (a five-nibble a-f codepoint spends its sixth
/// tap on the leading zero) and the Enter tap, each a press + a release frame
/// (`2 + 2 + 6 * 2 + 2 = 18`); the macOS surrogate-pair path needs `8 * 2 + 1 = 17`. Held at
/// 20 for two frames of headroom.
const UNICODE_FRAME_CAP: usize = 20;
/// Scans each frame is presented. At the ~1 kHz loop this comfortably exceeds the 10 ms
/// host poll, so every synthesised press and release is observed (mirrors the timed
/// engine's tap-frame dwell).
const FRAME_DWELL_SCANS: u16 = 12;

/// HID modifier byte — Left Control (bit 0).
const MOD_LCTRL: u8 = 1 << 0;
/// HID modifier byte — Left Shift (bit 1).
const MOD_LSHIFT: u8 = 1 << 1;
/// HID modifier byte — Left Alt, i.e. macOS's Left Option (bit 2).
const MOD_LALT: u8 = 1 << 2;
/// HID modifier byte — Right Alt, the WinCompose compose key (bit 6).
const MOD_RALT: u8 = 1 << 6;

/// HID usage — Enter (commits a WinCompose sequence).
const USAGE_ENTER: u8 = 0x28;
/// HID usage — Space (commits an IBus sequence).
const USAGE_SPACE: u8 = 0x2C;
/// HID usage — `U` (the IBus Ctrl+Shift+U trigger and the WinCompose `U` after Right Alt).
const USAGE_U: u8 = 0x18;

/// One synthesised frame: a modifier byte and a basic usage, presented together for
/// [`FRAME_DWELL_SCANS`] scans.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Frame {
    mods: u8,
    key: u8,
}

/// The neutral frame: no modifier, no key. Releases everything a run held.
const EMPTY_FRAME: Frame = Frame { mods: 0, key: 0 };

/// The codepoint table plus the in-flight player runtime.
struct UnicodeState {
    /// Host-uploaded codepoints; `0` (the default) is an empty slot that types nothing.
    map: [u32; UNICODE_MAP_SLOTS],
    /// The sequence the current send is playing back.
    frames: [Frame; UNICODE_FRAME_CAP],
    /// Frames in the current sequence (`0` = idle).
    len: u8,
    /// Index of the frame being presented.
    idx: u8,
    /// Scans left before the current frame retires.
    dwell: u16,
}

impl UnicodeState {
    const fn new() -> Self {
        Self {
            map: [0; UNICODE_MAP_SLOTS],
            frames: [EMPTY_FRAME; UNICODE_FRAME_CAP],
            len: 0,
            idx: 0,
            dwell: 0,
        }
    }

    /// The frame to present this scan (the empty frame once the sequence is exhausted).
    fn current(&self) -> Frame {
        if (self.idx as usize) < self.len as usize {
            self.frames[self.idx as usize]
        } else {
            EMPTY_FRAME
        }
    }
}

/// Unicode-input feature: the active OS mode, the RAM codepoint table behind the
/// established mutex/`RefCell` discipline, and a one-bit "is a send playing?" gate.
pub struct Unicode {
    state: Mutex<CriticalSectionRawMutex, RefCell<UnicodeState>>,
    /// Active OS input mode (`MODE_LINUX` / `MODE_MACOS` / `MODE_WINDOWS`). A lock-free
    /// atomic so the press-edge cycle and the kcp setter never take the state lock.
    mode: AtomicU8,
    /// Whether a sequence is mid-playback — the `active()` fast-path flag.
    sending: AtomicBool,
}

/// The singleton in the [`FEATURES`](crate::features::FEATURES) registry.
pub static UNICODE: Unicode = Unicode {
    state: Mutex::new(RefCell::new(UnicodeState::new())),
    mode: AtomicU8::new(MODE_LINUX),
    sending: AtomicBool::new(false),
};

impl Unicode {
    /// Advance the active OS mode (the `UNICODE_MODE_CYCLE` press edge), wrapping at the
    /// last mode.
    pub fn cycle_mode(&self) {
        // Inert while the feature is disabled, so the keycode changes nothing while off (the
        // kcp `SET_MODE` op stays ungated, so the GUI can still configure the mode).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        let next = (self.mode.load(Ordering::Relaxed) + 1) % UNICODE_MODE_COUNT;
        self.mode.store(next, Ordering::Relaxed);
    }

    /// Set the active OS mode (the kcp `SET_MODE` op). An out-of-range mode is rejected
    /// with `false`, leaving the mode unchanged.
    fn set_mode(&self, mode: u8) -> bool {
        if mode >= UNICODE_MODE_COUNT {
            return false;
        }
        self.mode.store(mode, Ordering::Relaxed);
        true
    }

    /// Upload codepoint `cp` into slot `n` (the kcp `SET_MAP` op). An out-of-range slot is
    /// rejected with `false`. The value is stored verbatim; validity is checked at send.
    fn set_map(&self, n: usize, cp: u32) -> bool {
        if n >= UNICODE_MAP_SLOTS {
            return false;
        }
        self.state.lock(|c| c.borrow_mut().map[n] = cp);
        true
    }

    /// Start emitting slot `n`'s codepoint through the active OS mode (the `UM(n)` press
    /// edge). Builds the key sequence into the player; later scans drain it. An empty slot
    /// or an invalid scalar value types nothing and leaves any in-flight send untouched.
    pub fn send(&self, n: u8) {
        // Inert while the feature is disabled, so a press never sets `sending` (which the
        // gated overlay/tick would otherwise drain on re-enable, typing a stale codepoint).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        let mode = self.mode.load(Ordering::Relaxed);
        let started = self.state.lock(|c| {
            let mut s = c.borrow_mut();
            let cp = if (n as usize) < UNICODE_MAP_SLOTS {
                s.map[n as usize]
            } else {
                0
            };
            if cp == 0 || !is_scalar(cp) {
                return false;
            }
            let len = build_sequence(mode, cp, &mut s.frames);
            if len == 0 {
                return false;
            }
            s.len = len as u8;
            s.idx = 0;
            s.dwell = FRAME_DWELL_SCANS;
            true
        });
        // Only a real send arms the gate, so pressing `UM` on an empty slot never aborts a
        // sequence already playing.
        if started {
            self.sending.store(true, Ordering::Relaxed);
        }
    }
}

impl Feature for Unicode {
    fn id(&self) -> FeatureId {
        FeatureId::Unicode
    }

    fn name(&self) -> &'static str {
        "Unicode"
    }

    /// One relaxed load: skipped entirely unless a sequence is playing.
    fn active(&self) -> bool {
        self.sending.load(Ordering::Relaxed)
    }

    /// Abort any in-flight codepoint send when the feature is switched off, so no
    /// half-typed sequence is stranded. The host-uploaded codepoint map and the active
    /// mode are configuration, not transient state, so they are left intact (disable is
    /// inert, never destructive).
    fn on_disable(&self) {
        self.sending.store(false, Ordering::Relaxed);
        self.state.lock(|c| {
            let mut s = c.borrow_mut();
            s.len = 0;
            s.idx = 0;
            s.dwell = 0;
        });
    }

    fn on_overlay(&self, _c: &Ctx, r: &mut Report) {
        if !self.sending.load(Ordering::Relaxed) {
            return;
        }
        let frame = self.state.lock(|c| c.borrow().current());
        if frame == EMPTY_FRAME {
            return;
        }
        r.boot.modifier |= frame.mods;
        emit_key(r, frame.key);
    }

    fn on_tick(&self, _now: Instant) {
        if !self.sending.load(Ordering::Relaxed) {
            return;
        }
        // Hold each frame for its dwell, then advance; the run ends when the last frame
        // (always an empty release frame) retires.
        let finished = self.state.lock(|c| {
            let mut s = c.borrow_mut();
            if s.dwell > 0 {
                s.dwell -= 1;
            }
            if s.dwell == 0 {
                s.idx = s.idx.saturating_add(1);
                if (s.idx as usize) >= s.len as usize {
                    s.len = 0;
                    s.idx = 0;
                    return true;
                }
                s.dwell = FRAME_DWELL_SCANS;
            }
            false
        });
        if finished {
            self.sending.store(false, Ordering::Relaxed);
        }
    }

    fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
        let status = match cmd {
            kcp::CMD_UNICODE_GET => {
                out[0] = self.mode.load(Ordering::Relaxed);
                out[1] = UNICODE_MAP_SLOTS as u8;
                out[2] = UNICODE_MODE_COUNT;
                Status::Ok
            }
            kcp::CMD_UNICODE_SET_MODE => {
                if self.set_mode(req[0]) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_UNICODE_SET_MAP => {
                let slot = req[0] as usize;
                let cp = u32::from_le_bytes([req[1], req[2], req[3], req[4]]);
                if self.set_map(slot, cp) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            // The first feature to claim its group owns it; an unrecognised op in the
            // Unicode group is BadCmd (not the Unsupported an unknown group gets), and any
            // other group is not ours.
            _ if cmd >> 4 == kcp::group::UNICODE => Status::BadCmd,
            _ => return None,
        };
        Some(status)
    }
}

/// Whether `cp` is a Unicode scalar value: in range (`<= 0x10FFFF`) and not a UTF-16
/// surrogate (`0xD800..=0xDFFF`), the two classes that cannot be typed as a character.
const fn is_scalar(cp: u32) -> bool {
    cp <= 0x10FFFF && !(cp >= 0xD800 && cp <= 0xDFFF)
}

/// HID keyboard usage for a hex nibble (`0..=0xF`): `DIGIT_0`, `DIGIT_1..DIGIT_9`, then `KEY_A..KEY_F`
/// — the digits every OS sender types.
const fn hex_usage(nibble: u8) -> u8 {
    match nibble & 0xF {
        0 => 0x27,                 // DIGIT_0
        d @ 1..=9 => 0x1E + d - 1, // DIGIT_1..DIGIT_9 (0x1E..=0x26)
        d => 0x04 + d - 0xA,       // KEY_A..KEY_F (0x04..=0x09)
    }
}

/// Append a frame, dropping it if the buffer is full (the capacity bounds every sequence,
/// so this never trips in practice).
fn push(frames: &mut [Frame; UNICODE_FRAME_CAP], n: &mut usize, frame: Frame) {
    if *n < UNICODE_FRAME_CAP {
        frames[*n] = frame;
        *n += 1;
    }
}

/// Append a tap — a press frame then its release — so the host sees a distinct down and up.
fn push_tap(frames: &mut [Frame; UNICODE_FRAME_CAP], n: &mut usize, mods: u8, key: u8) {
    push(frames, n, Frame { mods, key });
    push(frames, n, EMPTY_FRAME);
}

/// Append the codepoint's hex digits as taps, most-significant non-zero nibble first (and
/// always at least the final nibble). With `leading_zero` — the WinCompose
/// `needs_leading_zero` rule — a `0` is typed before the first digit when that digit is a-f,
/// which WinCompose requires to read the run as a hex sequence. Used by the IBus
/// (`leading_zero = false`) and WinCompose (`leading_zero = true`) senders, which accept a
/// variable-length hex string.
fn push_hex_min(frames: &mut [Frame; UNICODE_FRAME_CAP], n: &mut usize, cp: u32, leading_zero: bool) {
    // Six nibbles span the scalar range (0x10FFFF is 21 bits).
    let mut shift: i32 = 20;
    let mut started = false;
    while shift >= 0 {
        let nibble = ((cp >> shift) & 0xF) as u8;
        if nibble != 0 || started || shift == 0 {
            if !started && leading_zero && nibble > 9 {
                push_tap(frames, n, 0, hex_usage(0));
            }
            push_tap(frames, n, 0, hex_usage(nibble));
            started = true;
        }
        shift -= 4;
    }
}

/// Append a UTF-16 code `unit` as exactly four hex-digit taps, each carrying `hold` in both
/// the press *and* the release frame so the held modifier (macOS's Option) survives the
/// whole group. Used by the macOS sender, whose layout requires four digits per code unit.
fn push_hex4_held(frames: &mut [Frame; UNICODE_FRAME_CAP], n: &mut usize, hold: u8, unit: u16) {
    let mut shift: i32 = 12;
    while shift >= 0 {
        let nibble = ((unit >> shift) & 0xF) as u8;
        push(frames, n, Frame { mods: hold, key: hex_usage(nibble) });
        push(frames, n, Frame { mods: hold, key: 0 });
        shift -= 4;
    }
}

/// Build the active mode's key sequence for scalar value `cp` into `frames`, returning the
/// frame count. The caller guarantees `cp` is a valid scalar ([`is_scalar`]).
fn build_sequence(mode: u8, cp: u32, frames: &mut [Frame; UNICODE_FRAME_CAP]) -> usize {
    let mut n = 0;
    match mode {
        MODE_MACOS => {
            // Hold Option, type the UTF-16 code unit(s), release Option. BMP codepoints are
            // one four-digit unit; astral ones are a high+low surrogate pair.
            if cp <= 0xFFFF {
                push_hex4_held(frames, &mut n, MOD_LALT, cp as u16);
            } else {
                let v = cp - 0x10000;
                let high = (0xD800 + (v >> 10)) as u16;
                let low = (0xDC00 + (v & 0x3FF)) as u16;
                push_hex4_held(frames, &mut n, MOD_LALT, high);
                push_hex4_held(frames, &mut n, MOD_LALT, low);
            }
            push(frames, &mut n, EMPTY_FRAME);
        }
        MODE_WINDOWS => {
            // WinCompose: tap the compose key (Right Alt), tap U, type the hex (a leading 0
            // first when the first digit is a-f), commit with Enter.
            push_tap(frames, &mut n, MOD_RALT, 0);
            push_tap(frames, &mut n, 0, USAGE_U);
            push_hex_min(frames, &mut n, cp, true);
            push_tap(frames, &mut n, 0, USAGE_ENTER);
        }
        // MODE_LINUX and any unexpected value (the mode setters bound it) fall here.
        _ => {
            // IBus: tap Ctrl+Shift+U, type the hex, commit with Space.
            push_tap(frames, &mut n, MOD_LCTRL | MOD_LSHIFT, USAGE_U);
            push_hex_min(frames, &mut n, cp, false);
            push_tap(frames, &mut n, 0, USAGE_SPACE);
        }
    }
    n
}

/// Merge one basic usage into the report — the 6KRO boot slots and the NKRO bitmap — so an
/// injected key reaches the host on whichever rollover mode is negotiated. Mirrors the
/// timed engine's overlay merge; a `0` usage (the gap in a tap) is a no-op.
fn emit_key(report: &mut Report, usage: u8) {
    if usage == 0 {
        return;
    }
    if !report.boot.keycodes.contains(&usage) {
        if let Some(slot) = report.boot.keycodes.iter_mut().find(|s| **s == 0) {
            *slot = usage;
        } else {
            report.boot.keycodes = [keymap::ERROR_ROLL_OVER; 6];
        }
    }
    keymap::nkro_record(&mut report.nkro_bits, &mut report.high, usage);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// WinCompose types `RAlt`, `U`, the hex digits, then Enter — and prepends a `0` when the
    /// first hex digit is a-f (QMK's `register_hex32` `needs_leading_zero` rule). U+00E9
    /// (é = 0xE9) starts with `e`, so the digits are `0`, `e`, `9`. Each tap is a press frame
    /// then an empty release frame, so the presses are the even-indexed frames.
    #[test]
    fn windows_taps_ralt_u_leading_zero_hex_enter() {
        let mut frames = [EMPTY_FRAME; UNICODE_FRAME_CAP];
        let len = build_sequence(MODE_WINDOWS, 0x00E9, &mut frames);
        assert_eq!(len, 12);

        let press = |i: usize| (frames[i * 2].mods, frames[i * 2].key);
        assert_eq!(press(0), (MOD_RALT, 0)); // tap Right Alt
        assert_eq!(press(1), (0, USAGE_U)); // tap U
        assert_eq!(press(2), (0, hex_usage(0))); // leading zero (first digit `e` is a-f)
        assert_eq!(press(3), (0, hex_usage(0xE))); // e
        assert_eq!(press(4), (0, hex_usage(0x9))); // 9
        assert_eq!(press(5), (0, USAGE_ENTER)); // commit
        for i in (1..len).step_by(2) {
            assert_eq!((frames[i].mods, frames[i].key), (0, 0));
        }
    }

    /// A first hex digit of 0-9 needs no leading zero: U+0041 ('A' = 0x41) types `4`, `1`
    /// straight after the `RAlt`, `U` taps.
    #[test]
    fn windows_no_leading_zero_for_a_digit_first_nibble() {
        let mut frames = [EMPTY_FRAME; UNICODE_FRAME_CAP];
        let len = build_sequence(MODE_WINDOWS, 0x0041, &mut frames);
        assert_eq!(len, 10);

        let press = |i: usize| (frames[i * 2].mods, frames[i * 2].key);
        assert_eq!(press(0), (MOD_RALT, 0));
        assert_eq!(press(1), (0, USAGE_U));
        assert_eq!(press(2), (0, hex_usage(0x4))); // 4 — no leading zero
        assert_eq!(press(3), (0, hex_usage(0x1))); // 1
        assert_eq!(press(4), (0, USAGE_ENTER));
    }

    /// Every mode's worst-case codepoint stays within [`UNICODE_FRAME_CAP`]: the WinCompose
    /// leading-zero path (a five-nibble a-f codepoint) and the macOS surrogate pair are the
    /// two longest sequences.
    #[test]
    fn worst_case_sequences_fit_the_cap() {
        let mut frames = [EMPTY_FRAME; UNICODE_FRAME_CAP];
        // WinCompose, 0xFFFFF: leading zero + five digits → RAlt, U, 6 hex, Enter = 18.
        assert_eq!(build_sequence(MODE_WINDOWS, 0xF_FFFF, &mut frames), 18);
        // macOS astral codepoint: a high+low surrogate pair (8 hex) + the Option release = 17.
        assert_eq!(build_sequence(MODE_MACOS, 0x1_F600, &mut frames), 17);
    }
}
