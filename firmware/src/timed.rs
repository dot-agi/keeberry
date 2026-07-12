// SPDX-License-Identifier: GPL-2.0-or-later
//! Stateful, timed input behaviours: tap-dance, combos, dynamic macros, auto-shift,
//! the leader key, the mod-tap / layer-tap tap-hold keys, and Space-Cadet.
//!
//! Where [`crate::behavior`] carries the behaviours that are a pure function of a
//! single scan (SOCD, key overrides), this module carries the ones that need a
//! clock and a press/release state machine. A `TD(n)` key resolves differently
//! depending on how long it is held and how many times it is tapped; a combo
//! fires only when its whole key-set is pressed inside a time window; a macro
//! plays an event sequence out over wall-clock time; auto-shift sends a key's
//! shifted form when it is held past a timeout and its bare form on a quick tap;
//! the leader key opens a sequence the next keys are matched against; a mod-tap
//! (`MT`) / layer-tap (`LT`) key sends a basic key on a tap but holds modifiers or a
//! layer on a hold. None of that can be decided from one scan, so this is the
//! firmware's one *stateful* input layer.
//!
//! # Mod-tap and layer-tap
//!
//! `MT`/`LT` keycodes ([`crate::keycode`]) are gated through the keymap rather than an
//! engine table: [`keymap::tap_hold_present`](crate::keymap::tap_hold_present) keeps
//! [`engine_active`] true while the live keymap binds one, so the fast path is
//! preserved when none is bound. On press a key starts [`ThState::Pending`]; it
//! resolves to a tap (emitting `kc` as a frame, recorded for the quick-tap window) on
//! release within the term, or to a hold on term-expiry — asserting the modifiers
//! (`MT`, via the overlay's `hold_mods`) or activating the layer (`LT`, exposed by
//! [`momentary_layers`] for [`compute_report`](crate::keymap::compute_report) to fold
//! into its active mask). The global [`TapHoldTuning`] shifts the decision:
//! hold-on-other-key-press resolves to hold on any interrupting press, permissive-hold
//! on a nested press-and-release, retro-tapping emits the tap on a lone hold's release,
//! chordal hold settles a same-hand interrupt as a tap (so a same-hand roll types rather
//! than holding), and quick-tap repeats the tap when the key is re-pressed within its
//! window. Like the
//! other sub-engines the decision is the immediate-resolve style — a key pressed during
//! a pending tap-hold types under the layer/mods in force at its press (no full
//! key-buffering), so hold-on-other-key-press is the flavour to prefer for layer-taps.
//! Space-Cadet keys ride this same engine — a hold asserts the modifier, but a tap
//! emits a shifted symbol (a paren or Enter) rather than a bare key.
//!
//! # Auto-shift and the leader key
//!
//! Both are gated, like the engine tables, by [`TIMED_ANY`] — auto-shift contributes
//! to it through its runtime enable flag (default off), the leader key through a
//! non-empty sequence table — so an unconfigured keyboard pays nothing. Auto-shift
//! holds at most one key in its decision window: a quick tap replays the bare key,
//! a hold past [`as_timeout_ms`](TimedEngine::as_timeout_ms) asserts Left Shift with
//! it; a following key resolves the pending one first (bare if still inside the
//! window — so rolls stay lower-case), exactly as tap-dance interrupts. Its shift is
//! the report-wide modifier byte, so it cannot shift one held key while leaving a
//! second simultaneously-held one bare — the standard auto-shift bound. The leader
//! key (`LEADER`) captures up to [`MAX_LEADER_SEQ`] following key presses
//! (suppressed, so they do not type), restarting its timeout on each, and fires the
//! action of the first table entry whose whole sequence matches; an unmatched or
//! timed-out sequence is discarded. A leader action that is a `MACRO(n)` keycode
//! triggers that macro, so a leader sequence can play a full string.
//!
//! # Where it runs, and the two output channels
//!
//! [`crate::usb::keyboard_loop`] calls [`process`] on each debounced scan, right
//! after [`crate::matrix::Debouncer`] and *before* [`crate::keymap::compute_report`],
//! then [`apply_overlay`] right after it. The engine therefore straddles the
//! report builder with two channels:
//!
//! * **Matrix suppression** ([`process`]) returns the *effective* matrix — the
//!   physical scan with combo-claimed positions cleared — which is what
//!   [`compute_report`](crate::keymap::compute_report) actually resolves. A
//!   `TD`/`MACRO` position needs no suppression: those keycodes
//!   [`classify`](crate::keycode::Keycode::classify) as
//!   [`TapDance`](crate::keycode::KeyAction::TapDance) /
//!   [`Macro`](crate::keycode::KeyAction::Macro), which the report builder already
//!   ignores. Only combo members carry a real basic usage that must be hidden
//!   while the chord forms or fires.
//! * **HID injection** ([`apply_overlay`]) merges the *synthesised* output — a
//!   resolved tap-dance tap/hold, a fired combo's action, an in-flight macro
//!   frame — into the report. A synthesised keycode has no home matrix position,
//!   so it can only be injected, never resolved from the matrix.
//!
//! # Why it cannot stall the 1 kHz scan
//!
//! Both entry points are *synchronous* — there is no `.await` anywhere in this
//! module. All timing is read from [`embassy_time::Instant`] passed in from the
//! loop: a tap-dance / combo timeout is `now >= deadline`, a macro step is
//! `now >= next_step`. Each scan does work bounded by the (small, fixed) table
//! sizes and advances every state machine by at most the steps whose deadline has
//! passed — never a busy-loop, never a blocking wait. The cooperative executor is
//! free to run the USB and kcp tasks between scans.
//!
//! # State, borrows, and why they are sound
//!
//! All engine state lives in one [`TimedEngine`] behind a blocking
//! [`Mutex`]`<`[`CriticalSectionRawMutex`]`, `[`RefCell`]`>`, the same discipline
//! the live keymap and [`crate::behavior`] use. Every access — the keyboard loop
//! stepping the engine, or the kcp MACRO/BEHAVIOR groups writing a table — is a
//! synchronous critical section; the `RefCell` borrow is held only for the
//! few instructions of the step or store, and **no `.await` is ever held across a
//! borrow**. [`process`] resolves each edge's keycode through
//! [`keymap::resolve_keycode`](crate::keymap::resolve_keycode) (which takes the
//! `KEYMAP` lock) *before* it locks the engine, so the two mutexes are never
//! nested. Reader (keyboard loop) and writers (kcp loop) are futures on the same
//! cooperative thread-mode executor, which only switches tasks at an `.await`;
//! since no borrow spans one, a read and a write can never be live at once and the
//! `RefCell` is never borrowed re-entrantly.
//!
//! # Zero impact when unused
//!
//! A single [`AtomicBool`] [`TIMED_ANY`] shadows the engine's tables, tunables and
//! in-flight state, recomputed under the lock whenever any is mutated. With nothing
//! configured (the power-on default) [`process`] returns its input matrix untouched
//! and [`apply_overlay`] returns immediately, each after one relaxed load — so the
//! engine leaves the wired keyboard's report exactly as the keymap built it. A normal
//! keypress is never buffered or delayed: only a configured combo's own member keys
//! are ever held back, and only for that combo's term.
//!
//! # Persistence
//!
//! The tables are RAM-live, like [`crate::behavior`]'s: a host configures them
//! over kcp and they take effect on the next scan. They are persisted as part of
//! the full-config blob ([`crate::config`]) on `CONFIG.SAVE` and restored at boot.
//!
//! # Key safety invariants
//!
//! * **No stranded hold.** Per-key tap-dance state is keyed by the *physical*
//!   matrix position, not the keycode the position resolves to. A release edge is
//!   routed by position ([`TimedEngine::td_owner_at`]), so even if the layer the
//!   `TD` key lived on is released first — changing what the position now resolves
//!   to — the hold is still ended on the key's own physical release. (Same
//!   position-keying the combo suppression uses.)
//! * **No dropped real keypress.** A combo member's press is suppressed while the
//!   chord might still form; if the chord misses, the keystroke is replayed. The
//!   replay is all-or-nothing ([`TimedEngine::enqueue_tap`]) and, if the frame
//!   queue is momentarily full, the key stays buffered and is retried next scan
//!   ([`TimedEngine::pending_sweep`]) — never discarded. A still-held member is
//!   un-suppressed into the effective matrix instead, needing no synthesised frame.
//!
//! # Scope
//!
//! Tap-dance and combo *outputs* are basic keys and modifiers (so a tap-dance can
//! express mod-tap: hold a modifier, tap a key); a layer-switch output is out of
//! scope. A tap-dance with no double-tap action resolves its tap on release
//! (responsive); one *with* a double-tap action waits its term after release to
//! disambiguate a second tap — the standard latency cost of double-tap support.
//! Macro playback runs one sequence at a time (a retrigger restarts it). These
//! bounds are documented on the relevant items.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Duration, Instant};

use crate::behavior;
use crate::config;
use crate::features::{Ctx, Feature, FeatureId, FEATURE_ALWAYS_ON, FEATURE_DEFAULT_ON};
use crate::kcp::{self, Status};
use crate::keycode::{AutoShiftAction, KeyAction, Keycode};
use crate::keymap::{self, Report};
use crate::matrix::NUM_ROWS;
use usbd_hid::descriptor::KeyboardReport;

/// Maximum tap-dance entries the RAM table holds (the `TD(0..n)` range bound).
pub const MAX_TAP_DANCE: usize = 8;
/// Maximum combos the RAM table holds.
pub const MAX_COMBO: usize = 8;
/// Maximum keys in one combo's key-set.
pub const MAX_COMBO_KEYS: usize = 4;
/// Minimum keys in a valid combo (a single-key "combo" is just a key).
pub const MIN_COMBO_KEYS: usize = 2;
/// Maximum dynamic macros the RAM table holds (the `MACRO(0..n)` range bound).
pub const MAX_MACRO: usize = 4;
/// Maximum steps in one macro's sequence.
pub const MAX_MACRO_STEPS: usize = 32;
/// Maximum leader-sequence entries the RAM table holds.
pub const MAX_LEADER: usize = 8;
/// Maximum key presses captured in one leader sequence.
pub const MAX_LEADER_SEQ: usize = 5;

/// Default tap-dance decision window (ms), used until a host overrides it per
/// entry. Matches QMK's `TAPPING_TERM` default.
pub const DEFAULT_TAP_TERM_MS: u16 = 200;
/// Default combo window (ms), used until a host overrides it per combo. Matches
/// QMK's `COMBO_TERM` default.
pub const DEFAULT_COMBO_TERM_MS: u16 = 50;
/// Default auto-shift hold timeout (ms): a key held at least this long sends its
/// shifted form. Matches QMK's `AUTO_SHIFT_TIMEOUT` default.
pub const DEFAULT_AUTO_SHIFT_TIMEOUT_MS: u16 = 175;
/// Default leader inter-key timeout (ms): each captured key restarts it, and the
/// sequence ends when it elapses. Matches QMK's `LEADER_TIMEOUT` default.
pub const DEFAULT_LEADER_TIMEOUT_MS: u16 = 300;
/// Default mod-tap / layer-tap decision window (ms): held at least this long → hold,
/// released sooner → tap. Matches QMK's `TAPPING_TERM` default.
pub const DEFAULT_TAP_HOLD_TERM_MS: u16 = 200;
/// Default quick-tap window (ms): re-pressing a tap-hold key within this window of
/// its own tap repeats the tap (auto-repeat) instead of holding. Matches QMK's
/// `QUICK_TAP_TERM` default (`= TAPPING_TERM`); `0` disables quick-tap.
pub const DEFAULT_QUICK_TAP_TERM_MS: u16 = 200;

/// Held synthesised keys an overlay can assert at once (tap-dance holds + fired
/// combo actions). Six matches the boot report's key capacity.
const HOLD_CAP: usize = 6;
/// Pending one-shot tap frames (a resolved tap-dance tap, or a combo key dumped
/// because the chord broke). Sized past the worst single-scan dump — every
/// [`MAX_PENDING`] buffered combo member released at once, each a press+release
/// pair (`MAX_PENDING * 2 = 16`) — with headroom, so a combo miss never needs to
/// drop. A full ring never silently discards a real key anyway: [`enqueue_tap`]
/// is all-or-nothing and the combo-miss callers keep the key buffered and retry
/// next scan ([`backpressure`]) rather than losing it.
///
/// [`enqueue_tap`]: TimedEngine::enqueue_tap
/// [`backpressure`]: TimedEngine::pending_sweep
const FRAME_QUEUE_CAP: usize = 32;
/// Scans each queued tap frame is presented. At the ~1 kHz loop this is a few ms
/// — long enough that the 10 ms host poll observes each emitted press and release.
const FRAME_DWELL_SCANS: u16 = 12;
/// Live keys a playing macro can hold down at once.
const MACRO_LIVE_CAP: usize = 6;
/// Combo candidate positions buffered (suppressed) at once while chords form.
const MAX_PENDING: usize = 8;
/// Resolved key edges handled in one scan. Far above the handful of debounced
/// edges a single 1 ms scan can produce; extras (impossible in practice) are
/// dropped rather than growing the per-scan stack frame.
const MAX_EDGES: usize = 24;
/// Auto-shift keys that can be held shifted at once (each suppresses its matrix
/// position and asserts Left Shift). Six matches the boot report's key capacity.
const AS_HELD_CAP: usize = 6;
/// HID modifier byte for Left Shift (bit 1), the modifier auto-shift asserts for a
/// shifted key.
const AUTO_SHIFT_MOD: u8 = 1 << 1;
/// Mod-tap / layer-tap keys that can be pending or held at once. Eight comfortably
/// covers the realistic ceiling (home-row mods are four-or-so plus a thumb
/// layer-tap), well past the six-key boot report a held hold can ride; a press past
/// the cap falls back to emitting its tap so the keystroke is never lost.
const TAP_HOLD_CAP: usize = 8;

// ===========================================================================
// Configuration records (written by the kcp MACRO / BEHAVIOR groups)
// ===========================================================================

/// One tap-dance entry: the keycode for a single tap, the keycode for a hold, an
/// optional double-tap keycode ([`Keycode::NO`] when unset, falling back to the
/// tap keycode), and the per-entry decision window.
#[derive(Clone, Copy)]
pub struct TapDanceCfg {
    /// Emitted (as a tap) when the key is tapped once and released within the term.
    pub tap: Keycode,
    /// Held while the physical key is held, once the term elapses with it still down.
    pub hold: Keycode,
    /// Emitted (as a tap) on a double tap; [`Keycode::NO`] falls back to [`tap`].
    ///
    /// [`tap`]: Self::tap
    pub double: Keycode,
    /// Decision window in milliseconds (tap vs hold, and the double-tap window).
    pub tap_term_ms: u16,
}

/// One combo: a key-set of [`len`](Self::len) keycodes (`2..=MAX_COMBO_KEYS`), the
/// action keycode emitted while the whole set is held, the per-combo window and the
/// per-combo behaviour [`flags`](Self::flags).
#[derive(Clone, Copy)]
pub struct ComboCfg {
    /// The keycodes that must all be held together; only the first [`len`] are used.
    ///
    /// [`len`]: Self::len
    pub keys: [Keycode; MAX_COMBO_KEYS],
    /// Number of active entries in [`keys`](Self::keys) (`2..=MAX_COMBO_KEYS`).
    pub len: u8,
    /// Emitted (held, or tapped under [`FLAG_MUST_TAP`]) while the combo is fired.
    ///
    /// [`FLAG_MUST_TAP`]: Self::FLAG_MUST_TAP
    pub action: Keycode,
    /// Window in milliseconds: the assembly window (all keys pressed within it), or —
    /// under [`FLAG_MUST_HOLD`] / [`FLAG_MUST_TAP`] — the hold / tap decision window.
    ///
    /// [`FLAG_MUST_HOLD`]: Self::FLAG_MUST_HOLD
    /// [`FLAG_MUST_TAP`]: Self::FLAG_MUST_TAP
    pub term_ms: u16,
    /// Per-combo behaviour flags (see the `FLAG_*` constants); `0` is the default
    /// immediate-on-chord behaviour.
    pub flags: u8,
}

impl ComboCfg {
    /// Must-hold: the chord fires only once held for [`term_ms`](Self::term_ms);
    /// released sooner, the members type as individual keys (QMK's `COMBO_MUST_HOLD`).
    pub const FLAG_MUST_HOLD: u8 = 1 << 0;
    /// Must-tap: the chord fires (as a one-shot tap of the action) only when it is
    /// tapped — all members released within [`term_ms`](Self::term_ms); held longer,
    /// the members type as individual keys (QMK's `COMBO_MUST_TAP`).
    pub const FLAG_MUST_TAP: u8 = 1 << 1;
    /// In-order: the members must be pressed in the listed order for the chord to fire
    /// (QMK's `COMBO_MUST_PRESS_IN_ORDER`).
    pub const FLAG_IN_ORDER: u8 = 1 << 2;
    /// Every defined flag bit, so an unknown bit is rejected by the kcp setter.
    pub const FLAG_MASK: u8 = Self::FLAG_MUST_HOLD | Self::FLAG_MUST_TAP | Self::FLAG_IN_ORDER;

    /// Whether `kc` is one of this combo's first [`len`](Self::len) keys.
    fn contains(&self, kc: Keycode) -> bool {
        self.keys[..self.len as usize].contains(&kc)
    }

    /// Whether this combo waits a hold before firing ([`FLAG_MUST_HOLD`](Self::FLAG_MUST_HOLD)).
    fn must_hold(&self) -> bool {
        self.flags & Self::FLAG_MUST_HOLD != 0
    }

    /// Whether this combo fires only on a tap ([`FLAG_MUST_TAP`](Self::FLAG_MUST_TAP)).
    fn must_tap(&self) -> bool {
        self.flags & Self::FLAG_MUST_TAP != 0
    }

    /// Whether this combo requires in-order presses ([`FLAG_IN_ORDER`](Self::FLAG_IN_ORDER)).
    fn in_order(&self) -> bool {
        self.flags & Self::FLAG_IN_ORDER != 0
    }
}

/// One macro step: an event applied during playback. `down` presses the key (or
/// modifier); `!down` releases it. `delay_ms` is how long to dwell *after* the
/// step before the next one (the inter-event delay).
#[derive(Clone, Copy)]
pub struct MacroStep {
    /// Key or modifier the step presses or releases.
    pub kc: Keycode,
    /// `true` = press (down), `false` = release (up).
    pub down: bool,
    /// Delay in milliseconds before the next step is applied.
    pub delay_ms: u16,
}

impl MacroStep {
    const EMPTY: MacroStep = MacroStep {
        kc: Keycode::NO,
        down: false,
        delay_ms: 0,
    };
}

/// One dynamic macro: a fixed-capacity sequence of [`MacroStep`]s, `len` of them
/// active. An empty macro (`len == 0`) is an unconfigured slot.
#[derive(Clone, Copy)]
pub struct MacroCfg {
    steps: [MacroStep; MAX_MACRO_STEPS],
    len: u8,
}

impl MacroCfg {
    const EMPTY: MacroCfg = MacroCfg {
        steps: [MacroStep::EMPTY; MAX_MACRO_STEPS],
        len: 0,
    };
}

/// One leader-sequence entry: a key-press sequence of [`len`](Self::len) keycodes
/// (`1..=MAX_LEADER_SEQ`) and the action keycode fired when the whole sequence is
/// matched. An empty entry (`len == 0`) is an unconfigured slot.
#[derive(Clone, Copy)]
pub struct LeaderCfg {
    /// The keycodes that must be pressed in order after `LEADER`; only the first
    /// [`len`](Self::len) are used. Each is the keycode the pressed position resolves
    /// to under the active layers at the press.
    pub seq: [Keycode; MAX_LEADER_SEQ],
    /// Number of active entries in [`seq`](Self::seq) (`0` = empty, else
    /// `1..=MAX_LEADER_SEQ`).
    pub len: u8,
    /// Emitted when the sequence matches: a basic key / modifier is tapped, a
    /// `MACRO(n)` keycode triggers that macro.
    pub action: Keycode,
}

impl LeaderCfg {
    const EMPTY: LeaderCfg = LeaderCfg {
        seq: [Keycode::NO; MAX_LEADER_SEQ],
        len: 0,
        action: Keycode::NO,
    };
}

/// The global mod-tap / layer-tap tuning, configured over the kcp CONFIG TUNING group
/// and persisted in the config blob. These are the Vial-style global tap-hold knobs;
/// they apply to every `MT`/`LT` key (the keycode itself has no room for per-key
/// tuning), and the engine decides tap vs hold from them on each scan.
#[derive(Clone, Copy)]
pub struct TapHoldTuning {
    /// Decision window in ms: held at least this long resolves to hold, released
    /// sooner to tap. Must be non-zero (the kcp setter rejects `0`).
    pub term_ms: u16,
    /// Permissive hold: a *nested* press-and-release of another key while the tap-hold
    /// key is still held resolves it to hold on that release, even before the term.
    pub permissive_hold: bool,
    /// Hold on other key press: any other key pressed while the tap-hold key is held
    /// resolves it to hold immediately (so the other key lands under the hold).
    pub hold_on_other_key_press: bool,
    /// Retro tapping: a tap-hold key held past the term with no other key pressed
    /// still emits its tap when released (rather than a silent lone hold).
    pub retro_tapping: bool,
    /// Chordal hold (bilateral combinations): an interrupting key on the *same hand*
    /// as the tap-hold key settles it as a tap, so a same-hand roll types its key
    /// rather than triggering the hold; an opposite-hand interrupt (a cross-hand
    /// chord) defers to the flavours above. Handedness is the physical key geometry
    /// ([`keymap::key_hand`](crate::keymap::key_hand)).
    pub chordal_hold: bool,
    /// Quick-tap window in ms: re-pressing the tap-hold key within this window of its
    /// own tap repeats the tap (held as the basic key) instead of holding. `0`
    /// disables quick-tap (the second press always holds).
    pub quick_tap_term_ms: u16,
}

impl TapHoldTuning {
    /// The power-on defaults: QMK's tapping term, both interrupt flavours, retro and
    /// chordal hold off, quick-tap at the default window.
    const DEFAULT: TapHoldTuning = TapHoldTuning {
        term_ms: DEFAULT_TAP_HOLD_TERM_MS,
        permissive_hold: false,
        hold_on_other_key_press: false,
        retro_tapping: false,
        chordal_hold: false,
        quick_tap_term_ms: DEFAULT_QUICK_TAP_TERM_MS,
    };

    /// Bit positions of the boolean flavours within the single flags byte the kcp
    /// CONFIG TUNING group and the config blob both carry — one layout, so the wire
    /// byte and the stored byte can never disagree.
    const FLAG_PERMISSIVE: u8 = 1 << 0;
    const FLAG_HOLD_ON_OTHER: u8 = 1 << 1;
    const FLAG_RETRO: u8 = 1 << 2;
    const FLAG_CHORDAL: u8 = 1 << 3;

    /// Pack the boolean flavours into that flags byte.
    pub fn flags_byte(&self) -> u8 {
        let mut f = 0;
        if self.permissive_hold {
            f |= Self::FLAG_PERMISSIVE;
        }
        if self.hold_on_other_key_press {
            f |= Self::FLAG_HOLD_ON_OTHER;
        }
        if self.retro_tapping {
            f |= Self::FLAG_RETRO;
        }
        if self.chordal_hold {
            f |= Self::FLAG_CHORDAL;
        }
        f
    }

    /// Rebuild the tuning from the wire/blob fields — the term, the flags byte and the
    /// quick-tap window (`0` disables quick-tap) — the inverse of [`flags_byte`] plus
    /// the field reads.
    ///
    /// [`flags_byte`]: Self::flags_byte
    pub fn from_parts(term_ms: u16, flags: u8, quick_tap_term_ms: u16) -> Self {
        Self {
            term_ms,
            permissive_hold: flags & Self::FLAG_PERMISSIVE != 0,
            hold_on_other_key_press: flags & Self::FLAG_HOLD_ON_OTHER != 0,
            retro_tapping: flags & Self::FLAG_RETRO != 0,
            chordal_hold: flags & Self::FLAG_CHORDAL != 0,
            quick_tap_term_ms,
        }
    }
}

// ===========================================================================
// Runtime state
// ===========================================================================

/// Per-entry tap-dance runtime, advanced by [`process`].
#[derive(Clone, Copy)]
enum TdState {
    /// No interaction in progress.
    Idle,
    /// Press(es) seen; awaiting the term, a release, or an interrupt to resolve.
    Counting {
        /// Number of taps so far (`1` after the first press).
        count: u8,
        /// Timestamp of the most recent press/release edge; the term runs from here.
        last: Instant,
        /// Whether the key is currently physically held.
        held: bool,
        /// Physical position of the key (so a release of *this* key is matched).
        row: u8,
        col: u8,
    },
    /// Resolved to a hold: the hold keycode is asserted until the key is released.
    Held { row: u8, col: u8 },
}

/// One buffered combo candidate: a suppressed physical key that might complete a
/// combo. Held back from the effective matrix until the chord fires, breaks, or
/// the window passes.
#[derive(Clone, Copy)]
struct Pending {
    row: u8,
    col: u8,
    kc: Keycode,
    /// When the key was pressed (the combo window is measured from the first).
    press: Instant,
    /// Whether the key has since been released while still buffered.
    released: bool,
}

/// Per-combo runtime: whether it is currently fired, and the positions it claimed
/// when it fired (suppressed and holding the action until all are released).
#[derive(Clone, Copy)]
struct ComboRt {
    fired: bool,
    pos: [(u8, u8); MAX_COMBO_KEYS],
    npos: u8,
}

impl ComboRt {
    const NEW: ComboRt = ComboRt {
        fired: false,
        pos: [(0, 0); MAX_COMBO_KEYS],
        npos: 0,
    };
}

/// One synthesised tap frame (a chord presented for [`FRAME_DWELL_SCANS`] scans).
#[derive(Clone, Copy, PartialEq, Eq)]
struct Frame {
    mods: u8,
    key: u8,
}

impl Frame {
    const EMPTY: Frame = Frame { mods: 0, key: 0 };
}

/// A resolved physical key edge handed to [`TimedEngine::step`].
#[derive(Clone, Copy)]
struct Edge {
    row: u8,
    col: u8,
    pressed: bool,
    kc: Keycode,
}

impl Edge {
    const NONE: Edge = Edge {
        row: 0,
        col: 0,
        pressed: false,
        kc: Keycode::NO,
    };
}

/// One auto-shift key: the physical position, the basic HID usage it resolved to,
/// and the press instant the timeout runs from. Used both for the single key in the
/// decision window ([`as_decide`](TimedEngine::as_decide)) and for keys that timed
/// out into a shifted hold ([`as_held`](TimedEngine::as_held), where `press` is unused).
#[derive(Clone, Copy)]
struct AsKey {
    row: u8,
    col: u8,
    usage: u8,
    press: Instant,
}

/// An open leader sequence: the keycodes captured so far and the deadline the next
/// key must beat (restarted on each capture).
#[derive(Clone, Copy)]
struct LeaderRt {
    seq: [Keycode; MAX_LEADER_SEQ],
    len: u8,
    deadline: Instant,
}

/// What a tap-hold key holds: a modifier set (mod-tap `MT`) or a layer (layer-tap `LT`).
#[derive(Clone, Copy)]
enum ThKind {
    /// Mod-tap: hold asserts this HID modifier byte.
    Mod(u8),
    /// Layer-tap: hold momentarily activates this layer.
    Layer(u8),
}

/// The resolution state of one pending/held tap-hold key.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ThState {
    /// Undecided: awaiting the term, a release, or an interrupt.
    Pending,
    /// Resolved to hold: the modifiers/layer are asserted until release.
    Hold,
    /// Quick-tap auto-repeat: the tap key is held down (re-press inside the quick-tap
    /// window), so the host auto-repeats it instead of the key holding.
    Repeat,
}

/// One in-flight tap-hold (`MT`/`LT`) key, keyed by physical position so its release
/// is routed regardless of the active layer at release (like the tap-dance state).
#[derive(Clone, Copy)]
struct TapHoldRt {
    row: u8,
    col: u8,
    /// What a hold asserts (modifiers or a layer).
    kind: ThKind,
    /// The tap usage emitted on a tap / held in [`ThState::Repeat`].
    kc: u8,
    /// Modifier byte that rides the tap (`0` for plain mod-tap / layer-tap; the Shift
    /// bit for a Space-Cadet key, so a tap of a paren key sends the shifted symbol).
    tap_mods: u8,
    /// Press instant; the term and quick-tap windows run from here.
    press: Instant,
    state: ThState,
    /// Whether any other key was pressed while this key was pending — the retro-tap
    /// guard (retro only fires for a *lone* hold) and the permissive-hold trigger.
    saw_other: bool,
    /// Positions pressed *after* this key while it is pending (the nested set). A
    /// release of one of these, with permissive hold on, resolves this key to hold —
    /// keyed by position so a key held from *before* the tap-hold never counts.
    nested: [u16; NUM_ROWS],
}

/// The whole timed-behaviour engine: the three config tables, their runtime, and
/// the injection state. One instance behind [`TIMED`].
struct TimedEngine {
    // --- config tables ---
    td: [Option<TapDanceCfg>; MAX_TAP_DANCE],
    combos: [Option<ComboCfg>; MAX_COMBO],
    macros: [MacroCfg; MAX_MACRO],

    // --- tap-dance runtime ---
    td_rt: [TdState; MAX_TAP_DANCE],

    // --- combo runtime ---
    pending: [Option<Pending>; MAX_PENDING],
    combo_rt: [ComboRt; MAX_COMBO],

    // --- injection: continuously-held synthesised keys (rebuilt every scan) ---
    hold_mods: u8,
    hold_keys: [u8; HOLD_CAP],

    // --- injection: one-shot tap-frame ring ---
    queue: [Frame; FRAME_QUEUE_CAP],
    q_head: usize,
    q_len: usize,
    cur_frame: Frame,
    dwell: u16,

    // --- injection: single macro player ---
    play: Option<u8>,
    play_step: u8,
    play_next: Instant,
    play_mods: u8,
    play_keys: [u8; MACRO_LIVE_CAP],

    // --- capture: single macro recorder (on-board dynamic-macro RECORD) ---
    record: Option<u8>,
    rec_last: Instant,

    // --- auto-shift ---
    /// Runtime enable flag (default off); the `AUTO_SHIFT_*` keycodes and kcp toggle it.
    as_enabled: bool,
    /// Hold timeout in ms: a key held at least this long resolves shifted.
    as_timeout_ms: u16,
    /// The single key currently in the decision window (suppressed, undecided).
    as_decide: Option<AsKey>,
    /// Keys that timed out into a shifted hold: suppressed and asserting Left Shift
    /// with their usage until their physical release.
    as_held: [Option<AsKey>; AS_HELD_CAP],

    // --- leader key ---
    /// The configurable sequence -> action table the leader key matches against.
    leader_table: [LeaderCfg; MAX_LEADER],
    /// Inter-key timeout in ms (restarted on each captured key).
    leader_timeout_ms: u16,
    /// The open leader sequence, or `None` when no `LEADER` is in progress.
    leader: Option<LeaderRt>,
    /// Positions captured by a leader sequence, suppressed until they release so a
    /// matched-or-discarded sequence key never types. Independent of [`leader`] being
    /// active, so a sequence that ends while keys are still held keeps suppressing them.
    leader_supp: [Option<(u8, u8)>; MAX_LEADER_SEQ],

    // --- mod-tap / layer-tap ---
    /// In-flight tap-hold keys (pending, held, or quick-tap repeating).
    th: [Option<TapHoldRt>; TAP_HOLD_CAP],
    /// Layers held by hold-resolved layer-taps, rebuilt each scan and exposed via
    /// [`momentary_layers`] for [`crate::keymap::compute_report`] to fold into the
    /// active-layer mask.
    th_layers: u16,
    /// The global tap-hold tuning (term, interrupt flavours, retro, quick-tap).
    th_tuning: TapHoldTuning,
    /// The last tap-hold tap, `(row, col, when)`, for the quick-tap window: a re-press
    /// of the same position within the window repeats the tap instead of holding.
    th_last_tap: Option<(u8, u8, Instant)>,
}

impl TimedEngine {
    const fn new() -> Self {
        Self {
            td: [None; MAX_TAP_DANCE],
            combos: [None; MAX_COMBO],
            macros: [MacroCfg::EMPTY; MAX_MACRO],
            td_rt: [TdState::Idle; MAX_TAP_DANCE],
            pending: [None; MAX_PENDING],
            combo_rt: [ComboRt::NEW; MAX_COMBO],
            hold_mods: 0,
            hold_keys: [0; HOLD_CAP],
            queue: [Frame::EMPTY; FRAME_QUEUE_CAP],
            q_head: 0,
            q_len: 0,
            cur_frame: Frame::EMPTY,
            dwell: 0,
            play: None,
            play_step: 0,
            play_next: Instant::from_ticks(0),
            play_mods: 0,
            play_keys: [0; MACRO_LIVE_CAP],
            record: None,
            rec_last: Instant::from_ticks(0),
            as_enabled: false,
            as_timeout_ms: DEFAULT_AUTO_SHIFT_TIMEOUT_MS,
            as_decide: None,
            as_held: [None; AS_HELD_CAP],
            leader_table: [LeaderCfg::EMPTY; MAX_LEADER],
            leader_timeout_ms: DEFAULT_LEADER_TIMEOUT_MS,
            leader: None,
            leader_supp: [None; MAX_LEADER_SEQ],
            th: [None; TAP_HOLD_CAP],
            th_layers: 0,
            th_tuning: TapHoldTuning::DEFAULT,
            th_last_tap: None,
        }
    }

    // --- small injection helpers -------------------------------------------

    /// Enqueue a tap of `kc` (press frame + release frame), so the host sees a
    /// distinct down and up. Modifiers and basic keys only; other kinds are a
    /// no-op success.
    ///
    /// All-or-nothing: it enqueues **both** frames or neither, returning whether
    /// they fit. A partial enqueue (press without its release) could strand a
    /// synthesised key down, so a tap that does not fit is reported as *not* sent —
    /// the combo-miss callers then keep the key buffered and retry next scan, so a
    /// real keypress is never dropped (see [`pending_sweep`](Self::pending_sweep)).
    #[must_use]
    fn enqueue_tap(&mut self, kc: Keycode) -> bool {
        let frame = match kc.classify() {
            KeyAction::Modifier(bit) => Frame {
                mods: 1 << bit,
                key: 0,
            },
            KeyAction::Key(usage) => Frame { mods: 0, key: usage },
            _ => return true,
        };
        // Need room for the press *and* its release before committing either.
        if self.q_len + 2 > FRAME_QUEUE_CAP {
            return false;
        }
        self.push_frame(frame);
        self.push_frame(Frame::EMPTY);
        true
    }

    /// Enqueue a tap of an explicit `mods` + `key` frame (press then release), so
    /// auto-shift can present a shifted key (`mods` = Left Shift) as one tap. Like
    /// [`enqueue_tap`](Self::enqueue_tap) it is all-or-nothing, returning whether the
    /// pair fit; a key of `0` makes it a bare modifier tap.
    #[must_use]
    fn enqueue_tap_keyed(&mut self, mods: u8, key: u8) -> bool {
        if self.q_len + 2 > FRAME_QUEUE_CAP {
            return false;
        }
        self.push_frame(Frame { mods, key });
        self.push_frame(Frame::EMPTY);
        true
    }

    fn push_frame(&mut self, f: Frame) {
        if self.q_len >= FRAME_QUEUE_CAP {
            return;
        }
        let tail = (self.q_head + self.q_len) % FRAME_QUEUE_CAP;
        self.queue[tail] = f;
        self.q_len += 1;
    }

    fn pop_frame(&mut self) -> Option<Frame> {
        if self.q_len == 0 {
            return None;
        }
        let f = self.queue[self.q_head];
        self.q_head = (self.q_head + 1) % FRAME_QUEUE_CAP;
        self.q_len -= 1;
        Some(f)
    }

    /// Assert `kc` into the continuously-held set (`hold_mods`/`hold_keys`),
    /// deduplicated. Modifiers and basic keys only.
    fn hold_add(&mut self, kc: Keycode) {
        match kc.classify() {
            KeyAction::Modifier(bit) => self.hold_mods |= 1 << bit,
            KeyAction::Key(usage) => insert_key(&mut self.hold_keys, usage),
            _ => {}
        }
    }

    // --- tap-dance ----------------------------------------------------------

    fn td_press(&mut self, n: usize, row: u8, col: u8, now: Instant) {
        if n >= MAX_TAP_DANCE || self.td[n].is_none() {
            return;
        }
        self.td_rt[n] = match self.td_rt[n] {
            TdState::Counting { count, .. } => TdState::Counting {
                count: count.saturating_add(1),
                last: now,
                held: true,
                row,
                col,
            },
            // Idle, or a stale Held whose release we missed: start a fresh count.
            _ => TdState::Counting {
                count: 1,
                last: now,
                held: true,
                row,
                col,
            },
        };
    }

    /// Find the tap-dance entry that currently owns physical position
    /// `(row, col)` — the one whose runtime recorded it on press. Used to route a
    /// release edge by position, *not* by the keycode the position resolves to now:
    /// the active layer may have changed between press and release (e.g. the layer
    /// the `TD` key lived on was released first), so re-resolving the release edge
    /// could classify it as a different keycode and strand a held entry. Keying off
    /// the physical position — like the combo suppression — closes that hole.
    fn td_owner_at(&self, row: u8, col: u8) -> Option<usize> {
        for n in 0..MAX_TAP_DANCE {
            let owns = match self.td_rt[n] {
                TdState::Counting { row: r, col: c, .. } | TdState::Held { row: r, col: c } => {
                    r == row && c == col
                }
                TdState::Idle => false,
            };
            if owns {
                return Some(n);
            }
        }
        None
    }

    /// Record the physical release of tap-dance entry `n` at `(row, col)`. A
    /// counting entry only notes the release (and timestamps it for the double-tap
    /// window); the resolution itself is centralised in [`step`](Self::step) so the
    /// tap/hold/double decision and its frame-queue backpressure live in one place.
    /// A held entry's release ends its hold immediately. Matched by position, so it
    /// fires regardless of the current active layer.
    fn td_release(&mut self, n: usize, row: u8, col: u8, now: Instant) {
        if n >= MAX_TAP_DANCE {
            return;
        }
        match self.td_rt[n] {
            TdState::Counting {
                count,
                row: r,
                col: c,
                ..
            } if r == row && c == col => {
                self.td_rt[n] = TdState::Counting {
                    count,
                    last: now,
                    held: false,
                    row,
                    col,
                };
            }
            TdState::Held { row: r, col: c } if r == row && c == col => {
                self.td_rt[n] = TdState::Idle;
            }
            _ => {}
        }
    }

    /// Resolve a counting tap-dance entry now (term elapsed, released with no
    /// double-tap to disambiguate, or interrupted): a still-held key becomes its
    /// hold action; a released key its tap/double action.
    ///
    /// The tap branch is *backpressured*: if the frame queue cannot hold the
    /// press+release pair it leaves the entry `Counting` and returns, so
    /// [`step`](Self::step) retries next scan rather than dropping the tap. The hold
    /// branch never enqueues (it is asserted continuously while held), so it always
    /// completes.
    fn td_resolve(&mut self, n: usize) {
        let TdState::Counting {
            count, held, row, col, ..
        } = self.td_rt[n]
        else {
            return;
        };
        let Some(cfg) = self.td[n] else {
            self.td_rt[n] = TdState::Idle;
            return;
        };
        if held {
            self.td_rt[n] = TdState::Held { row, col };
        } else {
            let kc = if count >= 2 && cfg.double != Keycode::NO {
                cfg.double
            } else {
                cfg.tap
            };
            if self.enqueue_tap(kc) {
                self.td_rt[n] = TdState::Idle;
            }
            // Queue full: stay `Counting`; `step` retries next scan (never lost).
        }
    }

    /// Interrupt every counting entry except `except` (a key press other than that
    /// entry's own re-tap forces an immediate tap/hold decision — QMK-style).
    fn td_interrupt(&mut self, except: Option<usize>) {
        for n in 0..MAX_TAP_DANCE {
            if Some(n) == except {
                continue;
            }
            if matches!(self.td_rt[n], TdState::Counting { .. }) {
                self.td_resolve(n);
            }
        }
    }

    // --- combos -------------------------------------------------------------

    /// Whether `kc` is a member of any not-yet-fired combo (so its press should be
    /// buffered rather than emitted, to see if the chord completes).
    fn is_combo_candidate(&self, kc: Keycode) -> bool {
        self.combos.iter().enumerate().any(|(i, c)| {
            matches!(c, Some(cfg) if !self.combo_rt[i].fired && cfg.contains(kc))
        })
    }

    fn pending_push(&mut self, row: u8, col: u8, kc: Keycode, now: Instant) {
        if let Some(slot) = self.pending.iter_mut().find(|p| p.is_none()) {
            *slot = Some(Pending {
                row,
                col,
                kc,
                press: now,
                released: false,
            });
        }
        // A full buffer drops the candidate: it simply types normally (it is not
        // suppressed), the safe degradation.
    }

    fn pending_release(&mut self, row: u8, col: u8) {
        for e in self.pending.iter_mut().flatten() {
            if e.row == row && e.col == col {
                e.released = true;
            }
        }
    }

    /// Gather the pending slots holding each of `cfg`'s members, plus the earliest
    /// member press (the window/hold reference). With `require_held`, only a
    /// currently-held, not-yet-released member counts — the held-fire path; without
    /// it, a released member still counts — the must-tap path, where the chord is
    /// being tapped. Returns `None` if any member is missing from the buffer.
    fn combo_member_slots(
        &self,
        cfg: &ComboCfg,
        physical: &[u16; NUM_ROWS],
        require_held: bool,
    ) -> Option<([usize; MAX_COMBO_KEYS], Instant)> {
        let len = cfg.len as usize;
        let mut slots = [usize::MAX; MAX_COMBO_KEYS];
        let mut earliest = Instant::from_ticks(u64::MAX);
        for (m, &member) in cfg.keys[..len].iter().enumerate() {
            let found = self.pending.iter().position(|p| {
                matches!(p, Some(e)
                    if e.kc == member && (!require_held || (!e.released && held(physical, e.row, e.col))))
            })?;
            slots[m] = found;
            if let Some(e) = &self.pending[found] {
                earliest = earliest.min(e.press);
            }
        }
        Some((slots, earliest))
    }

    /// Whether the buffered members named by `slots` were pressed in their listed
    /// order (each no earlier than the previous) — the in-order combo gate.
    fn press_order_ok(&self, cfg: &ComboCfg, slots: &[usize; MAX_COMBO_KEYS]) -> bool {
        let mut prev = Instant::from_ticks(0);
        for &slot in &slots[..cfg.len as usize] {
            let Some(e) = &self.pending[slot] else {
                return false;
            };
            if e.press < prev {
                return false;
            }
            prev = e.press;
        }
        true
    }

    /// Claim the buffered positions named by `slots` for fired combo `i`: they leave
    /// `pending` and stay suppressed via [`ComboRt`] until they release.
    fn combo_claim(&mut self, i: usize, slots: &[usize; MAX_COMBO_KEYS], len: usize) {
        let mut rt = ComboRt::NEW;
        for &slot in &slots[..len] {
            if let Some(e) = self.pending[slot].take() {
                rt.pos[rt.npos as usize] = (e.row, e.col);
                rt.npos += 1;
            }
        }
        rt.fired = true;
        self.combo_rt[i] = rt;
    }

    /// Try to fire each unfired held-combo (default or must-hold) whose whole key-set
    /// is currently buffered and held. The default flavour fires as soon as the set is
    /// held within its assembly window; must-hold waits until the set has been held for
    /// the term (a quick tap of the same keys instead releases them, so the sweep types
    /// them individually). In-order additionally requires the listed press order. On
    /// firing, the claimed positions leave `pending` and the action begins to be held.
    /// Must-tap combos are handled by [`combo_fire_tap`](Self::combo_fire_tap).
    fn combo_fire(&mut self, physical: &[u16; NUM_ROWS], now: Instant) {
        for i in 0..MAX_COMBO {
            if self.combo_rt[i].fired {
                continue;
            }
            let Some(cfg) = self.combos[i] else { continue };
            if cfg.must_tap() {
                continue;
            }
            let Some((slots, earliest)) = self.combo_member_slots(&cfg, physical, true) else {
                continue;
            };
            if cfg.in_order() && !self.press_order_ok(&cfg, &slots) {
                continue;
            }
            let held_for = now.saturating_duration_since(earliest);
            let term = Duration::from_millis(cfg.term_ms as u64);
            if cfg.must_hold() {
                if held_for < term {
                    continue;
                }
            } else if held_for > term {
                continue;
            }
            self.combo_claim(i, &slots, cfg.len as usize);
        }
    }

    /// Try to fire each unfired must-tap combo: the chord was tapped — every member
    /// pressed within the window and now released — so emit the action as a one-shot
    /// tap and consume the members (they do not also type individually). A still-held
    /// or too-slow chord is left to the sweep, which types the members live / as taps.
    fn combo_fire_tap(&mut self, physical: &[u16; NUM_ROWS], now: Instant) {
        for i in 0..MAX_COMBO {
            if self.combo_rt[i].fired {
                continue;
            }
            let Some(cfg) = self.combos[i] else { continue };
            if !cfg.must_tap() {
                continue;
            }
            let Some((slots, earliest)) = self.combo_member_slots(&cfg, physical, false) else {
                continue;
            };
            if cfg.in_order() && !self.press_order_ok(&cfg, &slots) {
                continue;
            }
            // Past the window the chord is not a tap; leave it to the sweep.
            if now.saturating_duration_since(earliest) > Duration::from_millis(cfg.term_ms as u64) {
                continue;
            }
            let len = cfg.len as usize;
            let all_released = slots[..len]
                .iter()
                .all(|&s| matches!(&self.pending[s], Some(e) if e.released));
            if !all_released {
                continue;
            }
            // A one-shot tap of the action; if the frame queue is momentarily full,
            // retry next scan (the members stay buffered) so the tap is never lost.
            if self.enqueue_tap(cfg.action) {
                for &slot in &slots[..len] {
                    self.pending[slot] = None;
                }
            }
        }
    }

    /// Whether `kc` is a member of a still-viable must-tap combo this scan — every
    /// member still buffered and the window not yet passed — so [`pending_sweep`] keeps
    /// a released member buffered (rather than dumping it as a tap) until the chord
    /// resolves into a combo tap or expires.
    fn must_tap_owns(&self, kc: Keycode, physical: &[u16; NUM_ROWS], now: Instant) -> bool {
        for (i, c) in self.combos.iter().enumerate() {
            let Some(cfg) = c else { continue };
            if self.combo_rt[i].fired || !cfg.must_tap() || !cfg.contains(kc) {
                continue;
            }
            if let Some((_, earliest)) = self.combo_member_slots(cfg, physical, false) {
                if now.saturating_duration_since(earliest)
                    <= Duration::from_millis(cfg.term_ms as u64)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Sweep buffered candidates that did not fire this scan: a released one is
    /// dumped as a tap (the keystroke is preserved), an expired-but-held one is
    /// un-suppressed (it now types normally, a few ms late). A released member of a
    /// still-viable must-tap chord is kept buffered so the chord can still fire as a
    /// tap once its last member lifts.
    fn pending_sweep(&mut self, physical: &[u16; NUM_ROWS], now: Instant) {
        for idx in 0..MAX_PENDING {
            let Some(e) = self.pending[idx] else { continue };
            if e.released {
                // Still part of a forming must-tap chord: hold it back for
                // `combo_fire_tap` rather than dumping it as an individual tap.
                if self.must_tap_owns(e.kc, physical, now) {
                    continue;
                }
                // A combo member pressed *and released* without completing a chord:
                // its press was suppressed, so the only way the keystroke survives
                // is to emit it as a tap. If the frame queue is momentarily full,
                // leave it buffered and retry next scan — never drop it. (The slot
                // stays occupied, so at worst a later candidate is not buffered and
                // simply types live, the safe degradation.)
                if self.enqueue_tap(e.kc) {
                    self.pending[idx] = None;
                }
            } else {
                let term = self.candidate_term_ms(e.kc);
                if now.saturating_duration_since(e.press) > Duration::from_millis(term as u64) {
                    // Window passed without firing: stop holding it back. It is
                    // still physically held, so it now appears in the effective
                    // matrix and types live — no synthesised frame needed.
                    self.pending[idx] = None;
                }
            }
        }
    }

    /// Longest term among the combos `kc` belongs to (a candidate is held back
    /// until even the slowest combo it could form has timed out).
    fn candidate_term_ms(&self, kc: Keycode) -> u16 {
        let mut term = 0u16;
        for (i, c) in self.combos.iter().enumerate() {
            if let Some(cfg) = c {
                if !self.combo_rt[i].fired && cfg.contains(kc) {
                    term = term.max(cfg.term_ms);
                }
            }
        }
        if term == 0 {
            DEFAULT_COMBO_TERM_MS
        } else {
            term
        }
    }

    /// Drop the buffering of every candidate (an unrelated key press broke any
    /// forming chord): held ones become live in the matrix, released ones are
    /// dumped as taps in press order.
    fn pending_flush(&mut self, physical: &[u16; NUM_ROWS]) {
        for idx in 0..MAX_PENDING {
            let Some(e) = self.pending[idx] else { continue };
            if held(physical, e.row, e.col) {
                // Still held: leaving `pending` un-suppresses it, so it appears in
                // the effective matrix from this scan on and types live.
                self.pending[idx] = None;
            } else if self.enqueue_tap(e.kc) {
                // Already released: only a synthesised tap preserves the keystroke.
                self.pending[idx] = None;
            }
            // Released but the queue is full: keep it buffered and retry next scan,
            // so a real keypress is never dropped (backpressure, not loss).
        }
    }

    /// Release any fired combo whose claimed keys are all up; keep suppressing the
    /// rest. Returns nothing — the suppression mask is recomputed in [`Self::step`].
    fn combo_unfire(&mut self, physical: &[u16; NUM_ROWS]) {
        for rt in self.combo_rt.iter_mut() {
            if !rt.fired {
                continue;
            }
            let any_held = (0..rt.npos as usize).any(|k| {
                let (r, c) = rt.pos[k];
                held(physical, r, c)
            });
            if !any_held {
                *rt = ComboRt::NEW;
            }
        }
    }

    // --- macros -------------------------------------------------------------

    /// Begin (or restart) playback of macro `n`.
    fn macro_trigger(&mut self, n: usize, now: Instant) {
        if n >= MAX_MACRO || self.macros[n].len == 0 {
            return;
        }
        self.play = Some(n as u8);
        self.play_step = 0;
        self.play_next = now;
        self.play_mods = 0;
        self.play_keys = [0; MACRO_LIVE_CAP];
    }

    /// Advance the macro player: apply every step whose delay has elapsed this
    /// scan. Bounded by the macro length, so it always terminates. On completion
    /// the live set is cleared (a balanced macro will already have released its
    /// keys; this also frees an unbalanced one).
    fn macro_step(&mut self, now: Instant) {
        let Some(n) = self.play else { return };
        let n = n as usize;
        let len = self.macros[n].len as usize;
        let mut guard = 0;
        while self.play.is_some() && now >= self.play_next && guard <= MAX_MACRO_STEPS {
            guard += 1;
            let step_idx = self.play_step as usize;
            if step_idx >= len {
                // Done: free the player and any keys it still held.
                self.play = None;
                self.play_mods = 0;
                self.play_keys = [0; MACRO_LIVE_CAP];
                break;
            }
            let step = self.macros[n].steps[step_idx];
            self.macro_apply(step);
            self.play_step = self.play_step.saturating_add(1);
            self.play_next = now + Duration::from_millis(step.delay_ms as u64);
        }
    }

    fn macro_apply(&mut self, step: MacroStep) {
        match step.kc.classify() {
            KeyAction::Modifier(bit) => {
                if step.down {
                    self.play_mods |= 1 << bit;
                } else {
                    self.play_mods &= !(1 << bit);
                }
            }
            KeyAction::Key(usage) => {
                if step.down {
                    insert_key(&mut self.play_keys, usage);
                } else {
                    for s in self.play_keys.iter_mut() {
                        if *s == usage {
                            *s = 0;
                        }
                    }
                }
            }
            _ => {}
        }
    }

    // --- macro recording (on-board dynamic-macro capture) -------------------

    /// Begin recording into macro slot `n` (the kcp `MACRO_RECORD_START` op): wipe
    /// the slot, then capture every subsequent basic key / modifier edge as a step
    /// until [`record_stop`](Self::record_stop). `false`, recording nothing, if `n`
    /// is out of range.
    ///
    /// Recording is *passive*, exactly like QMK's dynamic macros: [`step`](Self::step)
    /// only observes each edge through [`record_edge`](Self::record_edge), so the keys
    /// pressed while recording still type live (the report is built from the matrix as
    /// always) and are captured at the same time.
    fn record_start(&mut self, n: usize, now: Instant) -> bool {
        if n >= MAX_MACRO {
            return false;
        }
        self.macros[n] = MacroCfg::EMPTY;
        self.record = Some(n as u8);
        self.rec_last = now;
        true
    }

    /// Stop recording (the kcp `MACRO_RECORD_STOP` op). A no-op when nothing is
    /// being recorded. The captured slot is left as-is — a still-held key recorded
    /// without its release is harmless, since [`macro_step`](Self::macro_step)
    /// frees any dangling live key when playback completes.
    fn record_stop(&mut self) {
        self.record = None;
    }

    /// Capture one physical key edge into the recording slot, if recording. Only
    /// basic keys and modifiers are recorded — the kinds [`macro_apply`](Self::macro_apply)
    /// can replay — so a macro / tap-dance / layer position is observed but never
    /// captured (which also stops a macro recording its own invocation: no recursion).
    ///
    /// The inter-event delay is reconstructed faithfully: the gap since the previous
    /// captured edge is written onto *that previous* step (whose `delay_ms` is the
    /// dwell before the next one), so playback reproduces the recorded timing — and
    /// because a human's edges are milliseconds apart, the steps never collapse into
    /// one scan the way an all-zero-delay upload would. A slot already at
    /// [`MAX_MACRO_STEPS`] silently drops further edges; recording stays active so a
    /// later stop still ends cleanly.
    ///
    /// Scope: each edge is recorded under the keycode it resolves to *at that edge*.
    /// In the rare case the active layer changes between a key's press and release,
    /// the two edges can resolve differently; the resulting unbalanced step is still
    /// safe (playback frees dangling keys on completion), matching the single-layer
    /// bound the rest of this module documents.
    fn record_edge(&mut self, kc: Keycode, down: bool, now: Instant) {
        let Some(n) = self.record else { return };
        if !matches!(kc.classify(), KeyAction::Modifier(_) | KeyAction::Key(_)) {
            return;
        }
        let m = &mut self.macros[n as usize];
        let len = m.len as usize;
        if len >= MAX_MACRO_STEPS {
            return;
        }
        if len > 0 {
            let gap = now.saturating_duration_since(self.rec_last).as_millis();
            m.steps[len - 1].delay_ms = gap.min(u16::MAX as u64) as u16;
        }
        m.steps[len] = MacroStep {
            kc,
            down,
            delay_ms: 0,
        };
        m.len = len as u8 + 1;
        self.rec_last = now;
    }

    // --- auto-shift ---------------------------------------------------------

    /// Start (or restart) the decision window for a freshly pressed auto-shiftable
    /// key. Any key already in the window is resolved first ([`autoshift_interrupt`]),
    /// so only one key is ever undecided — the same single-key discipline QMK uses.
    ///
    /// [`autoshift_interrupt`]: Self::autoshift_interrupt
    fn autoshift_press(&mut self, row: u8, col: u8, kc: Keycode, physical: &[u16; NUM_ROWS], now: Instant) {
        self.autoshift_interrupt(physical, now);
        if let KeyAction::Key(usage) = kc.classify() {
            self.as_decide = Some(AsKey {
                row,
                col,
                usage,
                press: now,
            });
        }
    }

    /// Resolve the key in the decision window because another key arrived. A no-op
    /// when nothing is undecided; otherwise it is [`settled`](Self::autoshift_settle).
    fn autoshift_interrupt(&mut self, physical: &[u16; NUM_ROWS], now: Instant) {
        if let Some(k) = self.as_decide.take() {
            self.autoshift_settle(k, physical, now);
        }
    }

    /// Settle a key that has left the decision window (interrupted by another key, or
    /// timed out). If it is still physically held: past the timeout it becomes a
    /// shifted hold ([`as_held`](Self::as_held)), inside the window it is simply
    /// un-suppressed so the matrix types it live in its bare form (a fast roll stays
    /// lower-case with no synthesised frame). If its release already landed this scan
    /// (edges are walked in position, not time, order) it emits a tap instead — shifted
    /// if held past the timeout — so the keystroke is never dropped.
    fn autoshift_settle(&mut self, k: AsKey, physical: &[u16; NUM_ROWS], now: Instant) {
        let shifted = self.autoshift_elapsed(&k, now);
        if held(physical, k.row, k.col) {
            if shifted {
                self.autoshift_hold(k);
            }
        } else {
            let mods = if shifted { AUTO_SHIFT_MOD } else { 0 };
            let _ = self.enqueue_tap_keyed(mods, k.usage);
        }
    }

    /// Whether key `k` has been held at least the auto-shift timeout as of `now`.
    fn autoshift_elapsed(&self, k: &AsKey, now: Instant) -> bool {
        now.saturating_duration_since(k.press) >= Duration::from_millis(self.as_timeout_ms as u64)
    }

    /// Move a timed-out key into the shifted-hold set. A full set drops the hold (the
    /// key, no longer suppressed, types bare instead) — the safe degradation.
    fn autoshift_hold(&mut self, k: AsKey) {
        if let Some(slot) = self.as_held.iter_mut().find(|s| s.is_none()) {
            *slot = Some(k);
        }
    }

    /// Whether physical position `(row, col)` is owned by the auto-shift engine — the
    /// undecided key or one of the shifted holds — so its release routes here.
    fn autoshift_owns(&self, row: u8, col: u8) -> bool {
        matches!(self.as_decide, Some(k) if k.row == row && k.col == col)
            || self
                .as_held
                .iter()
                .flatten()
                .any(|k| k.row == row && k.col == col)
    }

    /// Handle the release of an auto-shift key: the undecided key emits a tap (shifted
    /// if it was held past the timeout, else bare); a shifted hold stops being asserted.
    fn autoshift_release(&mut self, row: u8, col: u8, now: Instant) {
        if let Some(k) = self.as_decide {
            if k.row == row && k.col == col {
                self.as_decide = None;
                let mods = if self.autoshift_elapsed(&k, now) {
                    AUTO_SHIFT_MOD
                } else {
                    0
                };
                // Queue full: the tap is dropped (rare). A bare key could instead ride
                // the matrix, but it is already released, so a frame is the only route.
                let _ = self.enqueue_tap_keyed(mods, k.usage);
                return;
            }
        }
        for slot in self.as_held.iter_mut() {
            if matches!(slot, Some(k) if k.row == row && k.col == col) {
                *slot = None;
                return;
            }
        }
    }

    /// Resolve a key that reached the timeout while still in the decision window (no
    /// interrupt): [`settle`](Self::autoshift_settle) it. Called once per scan from
    /// [`step`](Self::step).
    fn autoshift_timeout(&mut self, physical: &[u16; NUM_ROWS], now: Instant) {
        let Some(k) = self.as_decide else {
            return;
        };
        if self.autoshift_elapsed(&k, now) {
            self.as_decide = None;
            self.autoshift_settle(k, physical, now);
        }
    }

    /// Clear all in-flight auto-shift state (undecided key + shifted holds). Used when
    /// the feature is disabled: the positions stop being suppressed, so a still-held
    /// key reverts to typing live from the matrix rather than stranding.
    fn autoshift_reset(&mut self) {
        self.as_decide = None;
        self.as_held = [None; AS_HELD_CAP];
    }

    // --- leader key ---------------------------------------------------------

    /// Open a leader sequence: arm capture with a fresh deadline. The `LEADER` key
    /// itself carries no HID usage, so nothing types for it.
    fn leader_start(&mut self, now: Instant) {
        self.leader = Some(LeaderRt {
            seq: [Keycode::NO; MAX_LEADER_SEQ],
            len: 0,
            deadline: now + Duration::from_millis(self.leader_timeout_ms as u64),
        });
    }

    /// Capture one key press into the open leader sequence: suppress its position
    /// (so it does not type), append its keycode, restart the deadline, then resolve.
    /// An exact table match fires its action and ends the sequence; a sequence no
    /// table entry can still complete — or one that has reached [`MAX_LEADER_SEQ`]
    /// with no exact match — ends discarded.
    fn leader_feed(&mut self, row: u8, col: u8, kc: Keycode, now: Instant) {
        self.leader_supp_add(row, col);
        let snapshot = {
            let Some(rt) = self.leader.as_mut() else {
                return;
            };
            if (rt.len as usize) < MAX_LEADER_SEQ {
                rt.seq[rt.len as usize] = kc;
                rt.len += 1;
            }
            rt.deadline = now + Duration::from_millis(self.leader_timeout_ms as u64);
            *rt
        };
        if let Some(action) = self.leader_match_exact(&snapshot) {
            self.leader = None;
            self.leader_fire(action, now);
        } else if !self.leader_has_prefix(&snapshot) || snapshot.len as usize >= MAX_LEADER_SEQ {
            self.leader = None;
        }
    }

    /// End an open leader sequence because its deadline elapsed: fire the action if
    /// the captured keys exactly match a table entry, otherwise discard.
    fn leader_timeout(&mut self, now: Instant) {
        let Some(rt) = self.leader.take() else {
            return;
        };
        if let Some(action) = self.leader_match_exact(&rt) {
            self.leader_fire(action, now);
        }
    }

    /// The action of the first table entry whose whole sequence equals the captured
    /// one, or `None`. An empty capture never matches.
    fn leader_match_exact(&self, rt: &LeaderRt) -> Option<Keycode> {
        let len = rt.len as usize;
        if len == 0 {
            return None;
        }
        self.leader_table
            .iter()
            .find(|e| e.len as usize == len && e.seq[..len] == rt.seq[..len])
            .map(|e| e.action)
    }

    /// Whether some table entry is strictly longer than the captured sequence and
    /// shares its prefix — i.e. the sequence could still grow into a match.
    fn leader_has_prefix(&self, rt: &LeaderRt) -> bool {
        let len = rt.len as usize;
        self.leader_table
            .iter()
            .any(|e| e.len as usize > len && e.seq[..len] == rt.seq[..len])
    }

    /// Emit a matched leader action: a `MACRO(n)` keycode triggers that macro,
    /// anything else is enqueued as a tap (basic key / modifier; other kinds no-op).
    fn leader_fire(&mut self, action: Keycode, now: Instant) {
        match action.classify() {
            KeyAction::Macro(n) => self.macro_trigger(n as usize, now),
            _ => {
                let _ = self.enqueue_tap(action);
            }
        }
    }

    /// Suppress a leader-captured position until it releases (deduplicated). A full
    /// set drops the suppression, so the key simply types live — the safe degradation.
    fn leader_supp_add(&mut self, row: u8, col: u8) {
        if self
            .leader_supp
            .iter()
            .flatten()
            .any(|&(r, c)| r == row && c == col)
        {
            return;
        }
        if let Some(slot) = self.leader_supp.iter_mut().find(|s| s.is_none()) {
            *slot = Some((row, col));
        }
    }

    /// Release a leader-captured position from suppression; returns whether it owned
    /// `(row, col)` (so the caller skips the other release routes).
    fn leader_supp_release(&mut self, row: u8, col: u8) -> bool {
        let mut owned = false;
        for slot in self.leader_supp.iter_mut() {
            if matches!(slot, Some((r, c)) if *r == row && *c == col) {
                *slot = None;
                owned = true;
            }
        }
        owned
    }

    // --- mod-tap / layer-tap ------------------------------------------------

    /// Begin a tap-hold (`MT`/`LT`) key press. A re-press of the same position inside
    /// the quick-tap window starts in [`ThState::Repeat`] (the tap key auto-repeats);
    /// otherwise it starts [`ThState::Pending`], awaiting the term, a release, or an
    /// interrupt. A full table falls back to emitting the tap, so the keystroke is
    /// never lost — the same safe degradation the other sub-engines use.
    fn taphold_press(&mut self, row: u8, col: u8, kind: ThKind, kc: u8, tap_mods: u8, now: Instant) {
        let repeat = self.th_quick_tap_match(row, col, now);
        let Some(slot) = self.th.iter_mut().find(|s| s.is_none()) else {
            let _ = self.emit_tap(tap_mods, kc);
            return;
        };
        *slot = Some(TapHoldRt {
            row,
            col,
            kind,
            kc,
            tap_mods,
            press: now,
            state: if repeat { ThState::Repeat } else { ThState::Pending },
            saw_other: false,
            nested: [0; NUM_ROWS],
        });
    }

    /// Emit a tap-hold key's tap. A plain mod-tap / layer-tap (`tap_mods == 0`) routes
    /// through [`enqueue_tap`](Self::enqueue_tap) so a modifier tap usage is classified
    /// into the modifier byte; a Space-Cadet tap (`tap_mods != 0`) routes through
    /// [`enqueue_tap_keyed`](Self::enqueue_tap_keyed) so the held Shift rides the symbol
    /// frame. Returns whether the tap fit the frame queue.
    #[must_use]
    fn emit_tap(&mut self, tap_mods: u8, kc: u8) -> bool {
        if tap_mods != 0 {
            self.enqueue_tap_keyed(tap_mods, kc)
        } else {
            self.enqueue_tap(Keycode::from_usage(kc))
        }
    }

    /// Whether a fresh press at `(row, col)` falls inside the quick-tap window of this
    /// key's own last tap. Consumes the record, so only the press immediately after a
    /// tap repeats.
    fn th_quick_tap_match(&mut self, row: u8, col: u8, now: Instant) -> bool {
        let qt = self.th_tuning.quick_tap_term_ms;
        if qt == 0 {
            return false;
        }
        if let Some((r, c, when)) = self.th_last_tap {
            if r == row
                && c == col
                && now.saturating_duration_since(when) < Duration::from_millis(qt as u64)
            {
                self.th_last_tap = None;
                return true;
            }
        }
        false
    }

    /// The tap-hold entry that owns physical position `(row, col)`, routed by position
    /// (not the keycode it now resolves to) so a release lands on the right entry even
    /// if the active layer changed since the press — the same position-keying the
    /// tap-dance and auto-shift sub-engines use.
    fn taphold_owner_at(&self, row: u8, col: u8) -> Option<usize> {
        self.th
            .iter()
            .position(|e| matches!(e, Some(t) if t.row == row && t.col == col))
    }

    /// Handle the physical release of tap-hold entry `i`. A pending key resolves as a
    /// tap (emitted, and recorded for the quick-tap window); a held key ends its hold,
    /// emitting a retro tap when retro is on and no other key intervened; a repeating
    /// key just lifts (its tap was held live).
    fn taphold_release(&mut self, i: usize, now: Instant) {
        let Some(t) = self.th[i] else { return };
        match t.state {
            ThState::Pending => {
                let _ = self.emit_tap(t.tap_mods, t.kc);
                self.th_last_tap = Some((t.row, t.col, now));
            }
            ThState::Hold => {
                if self.th_tuning.retro_tapping && !t.saw_other {
                    let _ = self.emit_tap(t.tap_mods, t.kc);
                    self.th_last_tap = Some((t.row, t.col, now));
                }
            }
            ThState::Repeat => {}
        }
        self.th[i] = None;
    }

    /// Record a fresh press of another key against every in-flight tap-hold. A pending
    /// key is marked interrupted (the retro guard and the permissive-hold nested set)
    /// and, with hold-on-other-key-press, resolved to hold at once so the other key
    /// lands under the hold; a key already held is only marked interrupted, so retro
    /// tapping (a *lone* hold) is disqualified once another key joins it. Called before
    /// the pressing key creates its own state, so it never marks itself.
    ///
    /// Chordal hold runs first: a pending tap-hold whose key is on the *same hand* as
    /// the interrupting key settles as a tap right here, pre-empting the hold-on-other /
    /// permissive / term paths — so a same-hand roll types rather than holding, while an
    /// opposite-hand (cross-hand) interrupt falls through to those flavours unchanged.
    fn taphold_other_press(&mut self, row: u8, col: u8, now: Instant) {
        // Chordal pass: settle same-hand pending keys as taps. Kept separate because
        // emitting the tap borrows the engine mutably (so the in-place loop below
        // cannot), and clearing the slot drops the entry from that loop's view.
        if self.th_tuning.chordal_hold {
            for i in 0..TAP_HOLD_CAP {
                let Some(t) = self.th[i] else { continue };
                if t.state == ThState::Pending
                    && keymap::key_hand(t.col as usize) == keymap::key_hand(col as usize)
                {
                    let _ = self.emit_tap(t.tap_mods, t.kc);
                    self.th_last_tap = Some((t.row, t.col, now));
                    self.th[i] = None;
                }
            }
        }
        let hold_on_other = self.th_tuning.hold_on_other_key_press;
        for t in self.th.iter_mut().flatten() {
            match t.state {
                ThState::Pending => {
                    t.saw_other = true;
                    t.nested[row as usize] |= 1 << col;
                    if hold_on_other {
                        t.state = ThState::Hold;
                    }
                }
                ThState::Hold => t.saw_other = true,
                ThState::Repeat => {}
            }
        }
    }

    /// Settle permissive hold: the release of `(row, col)` — pressed *after* a
    /// still-pending tap-hold, so it sits in that key's nested set — is the nested
    /// press-and-release permissive hold selects on, and resolves the tap-hold to
    /// hold. The bit is cleared regardless, so a stale nested key never lingers.
    fn taphold_other_release(&mut self, row: u8, col: u8) {
        let permissive = self.th_tuning.permissive_hold;
        for t in self.th.iter_mut().flatten() {
            if t.state == ThState::Pending && t.nested[row as usize] & (1 << col) != 0 {
                t.nested[row as usize] &= !(1 << col);
                if permissive {
                    t.state = ThState::Hold;
                }
            }
        }
    }

    /// Resolve any pending tap-hold whose decision window has elapsed to hold — the
    /// default term-based flavour, run once per scan.
    fn taphold_timeout(&mut self, now: Instant) {
        let term = Duration::from_millis(self.th_tuning.term_ms as u64);
        for t in self.th.iter_mut().flatten() {
            if t.state == ThState::Pending && now.saturating_duration_since(t.press) >= term {
                t.state = ThState::Hold;
            }
        }
    }

    /// Clear all in-flight tap-hold state: held mods/layers stop being asserted (a
    /// still-held key reverts to typing nothing rather than stranding), matching the
    /// auto-shift / leader resets done alongside it.
    fn taphold_reset(&mut self) {
        self.th = [None; TAP_HOLD_CAP];
        self.th_layers = 0;
        self.th_last_tap = None;
    }

    // --- the per-scan step --------------------------------------------------

    /// Advance every state machine by one scan and return the matrix-suppression
    /// mask (bits to clear from the physical scan before [`compute_report`]).
    fn step(&mut self, edges: &[Edge], physical: &[u16; NUM_ROWS], now: Instant) -> [u16; NUM_ROWS] {
        // 1. Apply edges: route presses/releases into the sub-engines, and let the
        //    recorder observe each one first (a no-op unless recording).
        for e in edges {
            self.record_edge(e.kc, e.pressed, now);
            if e.pressed {
                // An open leader sequence claims every following press (suppressed),
                // taking precedence over tap-dance / combo / auto-shift.
                if self.leader.is_some() {
                    self.leader_feed(e.row, e.col, e.kc, now);
                    continue;
                }
                // Every fresh press is an interrupt for any pending tap-hold (the
                // hold-on-other-press flavour, the permissive nested set, the retro
                // guard) — including another tap-hold's own press, marked here before
                // it creates its own state below.
                self.taphold_other_press(e.row, e.col, now);
                match e.kc.classify() {
                    // `LEADER`: resolve anything pending, then open a sequence.
                    KeyAction::Leader => {
                        self.autoshift_interrupt(physical, now);
                        self.td_interrupt(None);
                        self.pending_flush(physical);
                        self.leader_start(now);
                    }
                    KeyAction::Macro(n) => {
                        self.autoshift_interrupt(physical, now);
                        self.td_interrupt(None);
                        self.macro_trigger(n as usize, now);
                    }
                    KeyAction::TapDance(n) => {
                        // A new tap-dance press interrupts *other* counting entries.
                        self.autoshift_interrupt(physical, now);
                        self.td_interrupt(Some(n as usize));
                        self.td_press(n as usize, e.row, e.col, now);
                    }
                    // Mod-tap / layer-tap: resolve the other sub-engines (this is a
                    // fresh key) and open this key's own tap/hold decision. The
                    // position carries no basic usage, so it needs no suppression — it
                    // resolves to `ModTap`/`LayerTap`, which `compute_report` ignores;
                    // the tap is emitted as a frame and the hold via the overlay /
                    // momentary layer.
                    KeyAction::ModTap { mods, kc } => {
                        self.autoshift_interrupt(physical, now);
                        self.td_interrupt(None);
                        self.pending_flush(physical);
                        self.taphold_press(e.row, e.col, ThKind::Mod(mods), kc, 0, now);
                    }
                    KeyAction::LayerTap { layer, kc } => {
                        self.autoshift_interrupt(physical, now);
                        self.td_interrupt(None);
                        self.pending_flush(physical);
                        self.taphold_press(e.row, e.col, ThKind::Layer(layer), kc, 0, now);
                    }
                    // Space-Cadet rides the same tap-hold engine: a hold asserts the
                    // shift modifier, a tap emits the paren / Enter usage carrying its
                    // tap modifier (so the symbol is the shifted form).
                    KeyAction::SpaceCadet(role) => {
                        let (hold_mods, tap_kc, tap_mods) = role.resolve();
                        self.autoshift_interrupt(physical, now);
                        self.td_interrupt(None);
                        self.pending_flush(physical);
                        self.taphold_press(
                            e.row,
                            e.col,
                            ThKind::Mod(hold_mods),
                            tap_kc,
                            tap_mods,
                            now,
                        );
                    }
                    _ => {
                        if self.is_combo_candidate(e.kc) {
                            self.autoshift_interrupt(physical, now);
                            self.pending_push(e.row, e.col, e.kc, now);
                        } else if self.as_enabled && is_auto_shiftable(e.kc) {
                            // Open (or restart) the auto-shift decision window; it
                            // suppresses the key until tap/hold is decided.
                            self.autoshift_press(e.row, e.col, e.kc, physical, now);
                        } else {
                            // A normal key resolves anything pending and breaks any
                            // forming chord — it must never itself be delayed.
                            self.autoshift_interrupt(physical, now);
                            self.td_interrupt(None);
                            self.pending_flush(physical);
                        }
                    }
                }
            } else {
                // A release of any *other* key may complete a permissive nested
                // press-and-release and resolve a still-pending tap-hold to hold
                // (a no-op for this key's own entry, whose nested set never holds
                // its own position).
                self.taphold_other_release(e.row, e.col);
                // Route the release by physical position, not by the keycode it
                // resolves to *now* (the layer may have changed since the press):
                // an owning tap-hold, leader-suppressed, owning tap-dance or owning
                // auto-shift position clears its own state, otherwise it is a combo
                // candidate's release. This is what prevents a stranded hold.
                if let Some(i) = self.taphold_owner_at(e.row, e.col) {
                    self.taphold_release(i, now);
                } else if self.leader_supp_release(e.row, e.col) {
                    // A captured leader key lifted; it owned nothing else.
                } else if let Some(n) = self.td_owner_at(e.row, e.col) {
                    self.td_release(n, e.row, e.col, now);
                } else if self.autoshift_owns(e.row, e.col) {
                    self.autoshift_release(e.row, e.col, now);
                } else {
                    self.pending_release(e.row, e.col);
                }
            }
        }

        // 2. Fire any completed combo before sweeping/expiring candidates: the
        //    held-flavour (default / must-hold) chords first, then the must-tap
        //    chords (which fire as a one-shot tap once the chord is released).
        self.combo_fire(physical, now);
        self.combo_fire_tap(physical, now);
        // 3. Dump released candidates as taps; un-suppress expired held ones.
        self.pending_sweep(physical, now);
        // 4. Resolve counting tap-dances: on the term elapsing (held -> hold,
        //    released -> tap/double), and immediately on release when the entry has
        //    no double-tap action to wait for — so a plain tap/hold key (e.g. a
        //    mod-tap) is responsive instead of always paying the full term. An
        //    entry whose tap could not be enqueued (queue full) stays `Counting`
        //    and is retried here next scan, so a tap is never dropped.
        for n in 0..MAX_TAP_DANCE {
            if let TdState::Counting { last, held, .. } = self.td_rt[n] {
                let cfg = self.td[n];
                let term = cfg.map(|c| c.tap_term_ms).unwrap_or(DEFAULT_TAP_TERM_MS);
                let no_double = cfg.map(|c| c.double == Keycode::NO).unwrap_or(true);
                let timed_out =
                    now.saturating_duration_since(last) >= Duration::from_millis(term as u64);
                if timed_out || (!held && no_double) {
                    self.td_resolve(n);
                }
            }
        }
        // 4b. Resolve any pending tap-hold whose decision window elapsed to hold (the
        //     default term-based flavour; the interrupt flavours resolve on the edges
        //     above).
        self.taphold_timeout(now);
        // 5. Release fired combos whose keys are all up.
        self.combo_unfire(physical);
        // 5b. Resolve auto-shift / leader timers: a key held past the auto-shift
        //     timeout becomes a shifted hold; a leader sequence past its deadline
        //     fires its match or is discarded.
        self.autoshift_timeout(physical, now);
        if let Some(rt) = self.leader {
            if now >= rt.deadline {
                self.leader_timeout(now);
            }
        }

        // 6. Rebuild the continuously-held synthesised set from the resolved state.
        self.hold_mods = 0;
        self.hold_keys = [0; HOLD_CAP];
        for n in 0..MAX_TAP_DANCE {
            if let TdState::Held { .. } = self.td_rt[n] {
                if let Some(cfg) = self.td[n] {
                    self.hold_add(cfg.hold);
                }
            }
        }
        for i in 0..MAX_COMBO {
            if self.combo_rt[i].fired {
                if let Some(cfg) = self.combos[i] {
                    self.hold_add(cfg.action);
                }
            }
        }
        // Auto-shift holds assert Left Shift plus the held usage. The shift rides the
        // report-wide modifier byte, so a second simultaneously-held key is shifted
        // too — the standard auto-shift bound.
        for k in self.as_held.iter().flatten() {
            self.hold_mods |= AUTO_SHIFT_MOD;
            insert_key(&mut self.hold_keys, k.usage);
        }
        // Tap-hold output: a held mod-tap asserts its modifiers, a held layer-tap its
        // layer (folded into the active mask via `momentary_layers`), and a quick-tap
        // repeating key holds its tap usage down so the host auto-repeats it. Rebuilt
        // here each scan from the resolved state, like the tap-dance / combo holds.
        self.th_layers = 0;
        for i in 0..TAP_HOLD_CAP {
            let Some(t) = self.th[i] else { continue };
            match t.state {
                ThState::Hold => match t.kind {
                    ThKind::Mod(m) => self.hold_mods |= m,
                    ThKind::Layer(l) => self.th_layers |= 1 << l,
                },
                // A quick-tap repeat holds the tap usage down (plus its tap modifier, so
                // a repeating Space-Cadet paren keeps the shifted symbol).
                ThState::Repeat => {
                    self.hold_mods |= t.tap_mods;
                    insert_key(&mut self.hold_keys, t.kc);
                }
                ThState::Pending => {}
            }
        }

        // 7. Advance the tap-frame ring and the macro player (once per scan). An
        // explicitly queued frame — including the empty "release" frame a tap
        // enqueues after its press — dwells its full term, so every emitted tap
        // shows a clean press *and* release; only an exhausted queue drops to empty
        // with no dwell.
        if self.dwell == 0 {
            match self.pop_frame() {
                Some(f) => {
                    self.cur_frame = f;
                    self.dwell = FRAME_DWELL_SCANS;
                }
                None => self.cur_frame = Frame::EMPTY,
            }
        }
        if self.dwell > 0 {
            self.dwell -= 1;
        }
        self.macro_step(now);

        // 8. Build the suppression mask: buffered combo candidates + fired combo
        //    members + the auto-shift undecided/held keys + leader-captured keys.
        let mut mask = [0u16; NUM_ROWS];
        for e in self.pending.iter().flatten() {
            mask[e.row as usize] |= 1 << e.col;
        }
        for rt in self.combo_rt.iter() {
            if rt.fired {
                for k in 0..rt.npos as usize {
                    let (r, c) = rt.pos[k];
                    mask[r as usize] |= 1 << c;
                }
            }
        }
        if let Some(k) = self.as_decide {
            mask[k.row as usize] |= 1 << k.col;
        }
        for k in self.as_held.iter().flatten() {
            mask[k.row as usize] |= 1 << k.col;
        }
        for &(r, c) in self.leader_supp.iter().flatten() {
            mask[r as usize] |= 1 << c;
        }
        mask
    }

    /// Whether any synthesised output is live this scan (held key, in-flight frame,
    /// or playing macro). Lets [`apply_overlay`] skip the merge when idle. The
    /// auto-shift held keys fold into `hold_mods`/`hold_keys`, so they are covered.
    fn overlay_active(&self) -> bool {
        self.hold_mods != 0
            || self.hold_keys.iter().any(|&k| k != 0)
            || self.cur_frame != Frame::EMPTY
            || self.play.is_some()
            || self.play_mods != 0
            || self.play_keys.iter().any(|&k| k != 0)
    }

    /// Whether any table is configured, a recording is in progress, auto-shift is on,
    /// or a leader sequence is configured / mid-capture (the [`TIMED_ANY`] value).
    /// Recording must keep the fast-path flag set so [`process`] still feeds edges to
    /// [`record_edge`](Self::record_edge) before the slot's first step; auto-shift and
    /// leader likewise need [`process`] running to see the keys they act on. The
    /// in-flight auto-shift / leader state is included so a feature turned off
    /// mid-keystroke keeps running until its suppressed keys drain.
    fn any_configured(&self) -> bool {
        self.td.iter().any(Option::is_some)
            || self.combos.iter().any(Option::is_some)
            || self.macros.iter().any(|m| m.len != 0)
            || self.record.is_some()
            || self.as_enabled
            || self.as_decide.is_some()
            || self.as_held.iter().any(Option::is_some)
            || self.leader_table.iter().any(|l| l.len != 0)
            || self.leader.is_some()
            || self.leader_supp.iter().any(Option::is_some)
    }
}

/// Whether `kc` is a basic key auto-shift should defer: the printable letters,
/// number row and symbol keys that have a shifted form (`A..Z`, `1..0` and the
/// punctuation block). Modifiers, whitespace, navigation, function and engine keys
/// are never auto-shifted, so they type immediately as always.
fn is_auto_shiftable(kc: Keycode) -> bool {
    matches!(kc.classify(), KeyAction::Key(u)
        if (0x04..=0x1D).contains(&u)   // A..Z
        || (0x1E..=0x27).contains(&u)   // 1..0
        || (0x2D..=0x38).contains(&u))  // - = [ ] \ #~ ; ' ` , . /
}

/// Whether position `(row, col)` is set in `physical`.
#[inline]
fn held(physical: &[u16; NUM_ROWS], row: u8, col: u8) -> bool {
    physical[row as usize] & (1 << col) != 0
}

/// Insert `usage` into the first free (`0`) slot of `slots`, deduplicated. A `0`
/// usage or a full set is a no-op. Shared by the held-key set and the macro
/// player's live set.
fn insert_key(slots: &mut [u8], usage: u8) {
    if usage == 0 || slots.contains(&usage) {
        return;
    }
    if let Some(slot) = slots.iter_mut().find(|s| **s == 0) {
        *slot = usage;
    }
}

/// The engine, behind the established mutex/`RefCell` discipline.
static TIMED: Mutex<CriticalSectionRawMutex, RefCell<TimedEngine>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new(TimedEngine::new()));

/// Fast-path flag: is anything configured? Recomputed under the lock on every
/// table mutation, so the per-scan path is a single relaxed load while unused.
static TIMED_ANY: AtomicBool = AtomicBool::new(false);

/// Recompute [`TIMED_ANY`] from the live tables (takes the lock itself).
fn recompute_any() {
    let any = TIMED.lock(|cell| cell.borrow().any_configured());
    TIMED_ANY.store(any, Ordering::Relaxed);
}

/// Whether the timed feature must run this scan: either an engine table / auto-shift /
/// leader is configured ([`TIMED_ANY`]) or the live keymap binds a mod-tap / layer-tap
/// key ([`keymap::tap_hold_present`]). Two relaxed atomic loads — the idle fast path —
/// so a keyboard with neither pays nothing, while an `MT`/`LT` keymap keeps the
/// tap-hold engine running even with no other behaviour configured (`MT`/`LT` are
/// keymap keycodes, not an engine table, so they are gated through the keymap).
fn engine_active() -> bool {
    TIMED_ANY.load(Ordering::Relaxed) || keymap::tap_hold_present()
}

/// Layers held momentarily by hold-resolved layer-taps, for
/// [`crate::keymap::compute_report`] to fold into its active-layer mask (a held `LT`
/// activates its layer exactly like a held `MO`). Zero — one relaxed load, no engine
/// lock — when no tap-hold is configured/held, preserving the report's fast path.
pub fn momentary_layers() -> u16 {
    if !engine_active() {
        return 0;
    }
    TIMED.lock(|cell| cell.borrow().th_layers)
}

// ===========================================================================
// Per-scan entry points (the timed feature's matrix/overlay hooks call these;
// the keyboard loop drives them via `features::run_on_matrix`/`run_on_overlay`)
// ===========================================================================

/// Transform the debounced scan into the *effective* matrix the report builder
/// resolves: the physical scan with combo-claimed, auto-shift and leader-captured
/// positions suppressed, after advancing every state machine (tap-dance, combo,
/// macro, auto-shift, leader) by this scan.
///
/// `prev` is last scan's debounced matrix (for edge detection), `now` the scan
/// timestamp, `active` last scan's active-layer mask (to resolve each pressed
/// position's keycode). A no-op returning `physical` unchanged — after the
/// [`engine_active`] fast-path load — until a table is configured (or auto-shift /
/// leader is enabled, or the keymap binds a mod-tap / layer-tap), so the unconfigured
/// keyboard's report is byte-for-byte unchanged.
fn process(
    physical: [u16; NUM_ROWS],
    prev: [u16; NUM_ROWS],
    now: Instant,
    active: u16,
) -> [u16; NUM_ROWS] {
    if !engine_active() {
        return physical;
    }

    // Resolve each changed position's keycode first — `resolve_keycode` takes the
    // KEYMAP lock, which we must not hold nested inside the engine lock — into a
    // small bounded buffer.
    let mut edges = [Edge::NONE; MAX_EDGES];
    let mut n = 0;
    for r in 0..NUM_ROWS {
        let mut bits = physical[r] ^ prev[r];
        while bits != 0 {
            let c = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if n >= MAX_EDGES {
                break;
            }
            let pressed = physical[r] & (1 << c) != 0;
            edges[n] = Edge {
                row: r as u8,
                col: c as u8,
                pressed,
                kc: keymap::resolve_keycode(r, c, active),
            };
            n += 1;
        }
    }

    let mask = TIMED.lock(|cell| cell.borrow_mut().step(&edges[..n], &physical, now));

    let mut out = physical;
    for r in 0..NUM_ROWS {
        out[r] &= !mask[r];
    }
    out
}

/// Merge the engine's synthesised output — held tap-dance/combo/mod-tap keycodes, a
/// quick-tap repeated key, the in-flight tap frame, and the playing macro's live keys
/// — into `report`, covering both the 6KRO boot report and the NKRO bitmap (so the
/// same keys reach the host whichever rollover mode is negotiated). A no-op (the
/// [`engine_active`] fast-path load) until a table is configured or a tap-hold is
/// bound. Synchronous: the borrow never spans an `.await`. A held layer-tap's layer
/// is not overlaid here — it is folded into the active mask via [`momentary_layers`].
fn apply_overlay(report: &mut Report) {
    if !engine_active() {
        return;
    }
    TIMED.lock(|cell| {
        let e = cell.borrow();
        if !e.overlay_active() {
            return;
        }
        report.boot.modifier |= e.hold_mods | e.cur_frame.mods | e.play_mods;
        for &k in e.hold_keys.iter() {
            emit(report, k);
        }
        for &k in e.play_keys.iter() {
            emit(report, k);
        }
        emit(report, e.cur_frame.key);
    });
}

/// Merge one basic usage into both the boot report and the NKRO bitmap.
fn emit(report: &mut Report, usage: u8) {
    if usage == 0 {
        return;
    }
    merge_boot(&mut report.boot, usage);
    keymap::nkro_record(&mut report.nkro_bits, &mut report.high, usage);
}

/// Place `usage` into a free boot-report slot, or signal rollover overflow if the
/// six are full. Skips a usage already present (no duplicate).
fn merge_boot(report: &mut KeyboardReport, usage: u8) {
    if report.keycodes.contains(&usage) {
        return;
    }
    for slot in report.keycodes.iter_mut() {
        if *slot == 0 {
            *slot = usage;
            return;
        }
    }
    report.keycodes = [keymap::ERROR_ROLL_OVER; 6];
}

// ===========================================================================
// kcp accessors — tap-dance (BEHAVIOR group), combos (BEHAVIOR group),
// macros (MACRO group). All synchronous; each mutation refreshes TIMED_ANY.
// ===========================================================================

/// Configure tap-dance entry `index`. `false` (writing nothing) if out of range.
pub fn td_set(index: usize, cfg: TapDanceCfg) -> bool {
    if index >= MAX_TAP_DANCE {
        return false;
    }
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.td[index] = Some(cfg);
        e.td_rt[index] = TdState::Idle;
    });
    recompute_any();
    true
}

/// Read tap-dance entry `index`. `None` for an empty slot or out-of-range index.
pub fn td_get(index: usize) -> Option<TapDanceCfg> {
    if index >= MAX_TAP_DANCE {
        return None;
    }
    TIMED.lock(|cell| cell.borrow().td[index])
}

/// Clear tap-dance entry `index`. `false` if out of range.
pub fn td_clear(index: usize) -> bool {
    if index >= MAX_TAP_DANCE {
        return false;
    }
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.td[index] = None;
        e.td_rt[index] = TdState::Idle;
    });
    recompute_any();
    true
}

/// Clear every tap-dance entry.
pub fn td_clear_all() {
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.td = [None; MAX_TAP_DANCE];
        e.td_rt = [TdState::Idle; MAX_TAP_DANCE];
    });
    recompute_any();
}

/// Configure combo `index`. Returns `false`, writing nothing, if the index is out
/// of range, `len` is not in `MIN_COMBO_KEYS..=MAX_COMBO_KEYS`, or the key-set
/// contains a duplicate keycode (a chord must be distinct keys).
pub fn combo_set(index: usize, cfg: ComboCfg) -> bool {
    let len = cfg.len as usize;
    if index >= MAX_COMBO || !(MIN_COMBO_KEYS..=MAX_COMBO_KEYS).contains(&len) {
        return false;
    }
    // Reject an unknown flag bit or the contradictory must-hold + must-tap pair (a
    // chord cannot require both a hold and a tap), so a malformed request is a clear
    // BadArg rather than silently storing a flag the engine ignores.
    if cfg.flags & !ComboCfg::FLAG_MASK != 0
        || cfg.flags & (ComboCfg::FLAG_MUST_HOLD | ComboCfg::FLAG_MUST_TAP)
            == (ComboCfg::FLAG_MUST_HOLD | ComboCfg::FLAG_MUST_TAP)
    {
        return false;
    }
    // Reject a key-set with a duplicate keycode: a combo such as `[KEY_A, KEY_A]`
    // would fire on a single physical key, which is no chord. Only the first `len`
    // entries are live (the rest are unused padding), so the scan is bounded to them.
    for i in 1..len {
        if cfg.keys[..i].contains(&cfg.keys[i]) {
            return false;
        }
    }
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.combos[index] = Some(cfg);
        e.combo_rt[index] = ComboRt::NEW;
    });
    recompute_any();
    true
}

/// Read combo `index`. `None` for an empty slot or out-of-range index.
pub fn combo_get(index: usize) -> Option<ComboCfg> {
    if index >= MAX_COMBO {
        return None;
    }
    TIMED.lock(|cell| cell.borrow().combos[index])
}

/// Clear combo `index`. `false` if out of range.
pub fn combo_clear(index: usize) -> bool {
    if index >= MAX_COMBO {
        return false;
    }
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.combos[index] = None;
        e.combo_rt[index] = ComboRt::NEW;
    });
    recompute_any();
    true
}

/// Clear every combo.
pub fn combo_clear_all() {
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.combos = [None; MAX_COMBO];
        e.combo_rt = [ComboRt::NEW; MAX_COMBO];
    });
    recompute_any();
}

/// Set one step of macro `index`, extending its length to cover `step` if needed.
/// `false` if `index`/`step` is out of range.
pub fn macro_set_step(index: usize, step: usize, ev: MacroStep) -> bool {
    if index >= MAX_MACRO || step >= MAX_MACRO_STEPS {
        return false;
    }
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.macros[index].steps[step] = ev;
        if step as u8 >= e.macros[index].len {
            e.macros[index].len = step as u8 + 1;
        }
    });
    recompute_any();
    true
}

/// Read one step of macro `index`, plus the macro's active length. `None` if the
/// index/step is out of range.
pub fn macro_get_step(index: usize, step: usize) -> Option<(MacroStep, u8)> {
    if index >= MAX_MACRO || step >= MAX_MACRO_STEPS {
        return None;
    }
    TIMED.lock(|cell| {
        let e = cell.borrow();
        Some((e.macros[index].steps[step], e.macros[index].len))
    })
}

/// Clear macro `index` (length to zero). `false` if out of range.
pub fn macro_clear(index: usize) -> bool {
    if index >= MAX_MACRO {
        return false;
    }
    TIMED.lock(|cell| cell.borrow_mut().macros[index] = MacroCfg::EMPTY);
    recompute_any();
    true
}

/// Clear every macro.
pub fn macro_clear_all() {
    TIMED.lock(|cell| cell.borrow_mut().macros = [MacroCfg::EMPTY; MAX_MACRO]);
    recompute_any();
}

/// Bitmap of macro slots that are non-empty (bit `i` = macro `i` has steps), for
/// the host to "list" configured macros in one read.
pub fn macro_used_bitmap() -> u32 {
    TIMED.lock(|cell| {
        let e = cell.borrow();
        let mut bits = 0u32;
        for (i, m) in e.macros.iter().enumerate() {
            if m.len != 0 {
                bits |= 1 << i;
            }
        }
        bits
    })
}

/// Host-initiated macro playback (the kcp `MACRO_PLAY` op), so a macro can be
/// fired without a keymap binding. `false` if `index` is out of range or empty.
pub fn macro_play(index: usize) -> bool {
    if index >= MAX_MACRO {
        return false;
    }
    let ok = TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        if e.macros[index].len == 0 {
            return false;
        }
        let now = Instant::now();
        e.macro_trigger(index, now);
        true
    });
    if ok {
        // Playback uses the overlay path, which is gated on TIMED_ANY; a configured
        // macro already set it, but keep the invariant explicit.
        recompute_any();
    }
    ok
}

/// Start on-board recording into macro `index` (the kcp `MACRO_RECORD_START` op):
/// clears the slot and captures subsequent key edges as steps until
/// [`macro_record_stop`] is called. `false` if `index` is out of range. Refreshes
/// [`TIMED_ANY`] so [`process`] observes edges even before the first step lands.
pub fn macro_record_start(index: usize) -> bool {
    let ok = TIMED.lock(|cell| cell.borrow_mut().record_start(index, Instant::now()));
    if ok {
        recompute_any();
    }
    ok
}

/// Stop on-board recording (the kcp `MACRO_RECORD_STOP` op). Always succeeds; a
/// no-op if nothing was being recorded. Refreshes [`TIMED_ANY`] so a recording
/// that captured nothing into an otherwise-empty engine clears the fast-path flag.
pub fn macro_record_stop() {
    TIMED.lock(|cell| cell.borrow_mut().record_stop());
    recompute_any();
}

// ===========================================================================
// kcp / keymap accessors — auto-shift and the leader key.
// Auto-shift on/off + timeout and the leader timeout are runtime tunables (the
// CONFIG group); the leader sequence table is behaviour data (the BEHAVIOR group).
// All synchronous; each mutation refreshes TIMED_ANY.
// ===========================================================================

/// Apply an `AUTO_SHIFT_*` keycode press to the auto-shift enable flag (called by
/// [`keymap::compute_report`](crate::keymap::compute_report) on the always-run report
/// path, so it works even with nothing else configured). Disabling clears any
/// in-flight decision/hold so no key is left suppressed.
pub fn autoshift_control(action: AutoShiftAction) {
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.as_enabled = match action {
            AutoShiftAction::Toggle => !e.as_enabled,
            AutoShiftAction::On => true,
            AutoShiftAction::Off => false,
        };
        if !e.as_enabled {
            e.autoshift_reset();
        }
    });
    recompute_any();
}

/// Read the auto-shift tunables: `(enabled, timeout_ms)`. For the kcp CONFIG
/// tuning read and the config-blob snapshot.
pub fn autoshift_get() -> (bool, u16) {
    TIMED.lock(|cell| {
        let e = cell.borrow();
        (e.as_enabled, e.as_timeout_ms)
    })
}

/// Set the auto-shift tunables. `timeout_ms` is stored as given (the caller
/// validates it non-zero); disabling clears any in-flight decision/hold.
pub fn autoshift_set(enabled: bool, timeout_ms: u16) {
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.as_enabled = enabled;
        e.as_timeout_ms = timeout_ms;
        if !enabled {
            e.autoshift_reset();
        }
    });
    recompute_any();
}

/// Read the global mod-tap / layer-tap tuning. For the kcp CONFIG tuning read and the
/// config-blob snapshot.
pub fn taphold_get() -> TapHoldTuning {
    TIMED.lock(|cell| cell.borrow().th_tuning)
}

/// Set the global mod-tap / layer-tap tuning (the caller validates `term_ms` non-zero;
/// `quick_tap_term_ms == 0` is valid and disables quick-tap). It does not by itself arm
/// the engine — `MT`/`LT` are gated through the keymap ([`keymap::tap_hold_present`]) —
/// so [`TIMED_ANY`] is not recomputed, matching [`leader_set_timeout`].
pub fn taphold_set(tuning: TapHoldTuning) {
    TIMED.lock(|cell| cell.borrow_mut().th_tuning = tuning);
}

/// Read the leader inter-key timeout in ms. For the kcp CONFIG tuning read and the
/// config-blob snapshot.
pub fn leader_timeout_ms() -> u16 {
    TIMED.lock(|cell| cell.borrow().leader_timeout_ms)
}

/// Set the leader inter-key timeout in ms (the caller validates it non-zero). The
/// timeout alone does not arm the engine, so [`TIMED_ANY`] is not recomputed.
pub fn leader_set_timeout(timeout_ms: u16) {
    TIMED.lock(|cell| cell.borrow_mut().leader_timeout_ms = timeout_ms);
}

/// Configure leader entry `index`. Returns `false`, writing nothing, if the index is
/// out of range or `len` exceeds [`MAX_LEADER_SEQ`]. A `len` of `0` clears the slot.
pub fn leader_set(index: usize, cfg: LeaderCfg) -> bool {
    if index >= MAX_LEADER || cfg.len as usize > MAX_LEADER_SEQ {
        return false;
    }
    // Normalise before storing: the host may pad `seq`/`action` past `len`, and a
    // verbatim store would let `pack_leader` echo and the config blob persist that
    // padding — breaking the "empty slot / unused tail reads zeroed" contract both
    // rely on. Clear a `len == 0` slot to `EMPTY`; otherwise zero the unused tail.
    let cfg = if cfg.len == 0 {
        LeaderCfg::EMPTY
    } else {
        let mut cfg = cfg;
        cfg.seq[cfg.len as usize..].fill(Keycode::NO);
        cfg
    };
    TIMED.lock(|cell| cell.borrow_mut().leader_table[index] = cfg);
    recompute_any();
    true
}

/// Read leader entry `index`. `None` for an out-of-range index; a configured slot
/// has `len >= 1`, an empty one `len == 0`.
pub fn leader_get(index: usize) -> Option<LeaderCfg> {
    if index >= MAX_LEADER {
        return None;
    }
    Some(TIMED.lock(|cell| cell.borrow().leader_table[index]))
}

/// Clear every leader entry.
pub fn leader_clear_all() {
    TIMED.lock(|cell| cell.borrow_mut().leader_table = [LeaderCfg::EMPTY; MAX_LEADER]);
    recompute_any();
}

/// Reset the runtime tunables to their power-on defaults: auto-shift off at the
/// default timeout, the default leader timeout, an empty leader table and the default
/// tap-hold tuning. Backs the tunable half of [`crate::config::reset_to_defaults`]; the
/// in-flight auto-shift / leader / tap-hold state is cleared too.
pub fn reset_tunables() {
    TIMED.lock(|cell| {
        let mut e = cell.borrow_mut();
        e.as_enabled = false;
        e.as_timeout_ms = DEFAULT_AUTO_SHIFT_TIMEOUT_MS;
        e.leader_timeout_ms = DEFAULT_LEADER_TIMEOUT_MS;
        e.leader_table = [LeaderCfg::EMPTY; MAX_LEADER];
        e.th_tuning = TapHoldTuning::DEFAULT;
        e.autoshift_reset();
        e.taphold_reset();
        e.leader = None;
        e.leader_supp = [None; MAX_LEADER_SEQ];
    });
    recompute_any();
}

// ===========================================================================
// Feature impl
// ===========================================================================
//
// The timed engine is one registry feature. Its hooks delegate to the entry
// points and accessors above (which own the engine, its lock and the `TIMED_ANY`
// gate), so the registry is purely the call seam: `active` mirrors `engine_active`,
// `on_matrix`/`on_overlay` run `process`/`apply_overlay`, `on_kcp` owns the MACRO
// group and the timed half of the BEHAVIOR group (the tap-dance, combo and leader
// ops) and, as the registry's last feature, is the BadCmd catch-all for an
// unrecognised op in either group, and `on_save`/`on_load` defer to the fixed-offset
// blob logic in [`crate::config`].

/// The timed-behaviour feature: tap-dance, combos, dynamic macros, leader,
/// auto-shift and mod-tap / layer-tap, gated as a whole by [`engine_active`].
pub struct Timed;

impl Feature for Timed {
    fn id(&self) -> FeatureId {
        FeatureId::Timed
    }

    fn name(&self) -> &'static str {
        "Timed Engine"
    }

    /// Always-on: the timed engine backs core keycodes — mod-tap, layer-tap,
    /// tap-dance, combos, dynamic macros, leader, auto-shift — that a keymap can
    /// hard-depend on, so disabling it at runtime would strand those bindings. It is
    /// structural, not a user-toggleable add-on.
    fn flags(&self) -> u8 {
        FEATURE_DEFAULT_ON | FEATURE_ALWAYS_ON
    }

    /// Mirrors the [`engine_active`] fast-path gate, so the dispatcher skips the
    /// matrix/overlay hooks for a couple of relaxed loads until a table is configured
    /// (or auto-shift / leader is enabled, or the keymap binds a mod-tap / layer-tap).
    fn active(&self) -> bool {
        engine_active()
    }

    fn on_matrix(&self, c: &Ctx, m: &mut [u16; NUM_ROWS]) {
        *m = process(*m, *c.prev_matrix, c.now, c.active_layers);
    }

    fn on_overlay(&self, _c: &Ctx, r: &mut Report) {
        apply_overlay(r);
    }

    fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
        let status = match cmd {
            // --- MACRO group (0x5x) — Timed owns it entirely ---
            kcp::CMD_MACRO_INFO => {
                out[0] = MAX_MACRO as u8;
                out[1] = MAX_MACRO_STEPS as u8;
                out[2..6].copy_from_slice(&macro_used_bitmap().to_le_bytes());
                Status::Ok
            }
            kcp::CMD_MACRO_SET_STEP => {
                let macro_idx = req[0] as usize;
                let step_idx = req[1] as usize;
                let ev = MacroStep {
                    kc: kc_le(req, 2),
                    down: req[4] != 0,
                    delay_ms: u16::from_le_bytes([req[5], req[6]]),
                };
                if macro_set_step(macro_idx, step_idx, ev) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_MACRO_GET_STEP => {
                let macro_idx = req[0] as usize;
                let step_idx = req[1] as usize;
                match macro_get_step(macro_idx, step_idx) {
                    Some((step, len)) => {
                        out[0] = (step_idx < len as usize) as u8;
                        out[1..3].copy_from_slice(&step.kc.raw().to_le_bytes());
                        out[3] = step.down as u8;
                        out[4..6].copy_from_slice(&step.delay_ms.to_le_bytes());
                        out[6] = len;
                        Status::Ok
                    }
                    None => Status::BadArg,
                }
            }
            kcp::CMD_MACRO_CLEAR => {
                let index = req[0];
                if index == behavior::CLEAR_ALL {
                    macro_clear_all();
                    Status::Ok
                } else if macro_clear(index as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_MACRO_PLAY => {
                if macro_play(req[0] as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_MACRO_RECORD_START => {
                if macro_record_start(req[0] as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_MACRO_RECORD_STOP => {
                macro_record_stop();
                Status::Ok
            }
            // --- BEHAVIOR group (0x7x) — the timed ops only ---
            kcp::CMD_TAPDANCE_SET => {
                let index = req[0] as usize;
                let cfg = TapDanceCfg {
                    tap: kc_le(req, 1),
                    hold: kc_le(req, 3),
                    double: kc_le(req, 5),
                    tap_term_ms: u16::from_le_bytes([req[7], req[8]]),
                };
                if td_set(index, cfg) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_TAPDANCE_GET => {
                let index = req[0] as usize;
                if index >= MAX_TAP_DANCE {
                    Status::BadArg
                } else {
                    pack_tapdance(td_get(index), out);
                    Status::Ok
                }
            }
            kcp::CMD_TAPDANCE_CLEAR => {
                let index = req[0];
                if index == behavior::CLEAR_ALL {
                    td_clear_all();
                    Status::Ok
                } else if td_clear(index as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_COMBO_SET => {
                let index = req[0] as usize;
                let cfg = ComboCfg {
                    len: req[1],
                    keys: [
                        kc_le(req, 2),
                        kc_le(req, 4),
                        kc_le(req, 6),
                        kc_le(req, 8),
                    ],
                    action: kc_le(req, 10),
                    term_ms: u16::from_le_bytes([req[12], req[13]]),
                    flags: req[14],
                };
                if combo_set(index, cfg) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_COMBO_GET => {
                let index = req[0] as usize;
                if index >= MAX_COMBO {
                    Status::BadArg
                } else {
                    pack_combo(combo_get(index), out);
                    Status::Ok
                }
            }
            kcp::CMD_COMBO_CLEAR => {
                let index = req[0];
                if index == behavior::CLEAR_ALL {
                    combo_clear_all();
                    Status::Ok
                } else if combo_clear(index as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_TIMED_INFO => {
                out[0] = MAX_TAP_DANCE as u8;
                out[1] = MAX_COMBO as u8;
                out[2] = MAX_COMBO_KEYS as u8;
                out[3] = MAX_MACRO as u8;
                out[4] = MAX_MACRO_STEPS as u8;
                out[5] = MAX_LEADER as u8;
                out[6] = MAX_LEADER_SEQ as u8;
                Status::Ok
            }
            kcp::CMD_LEADER_SET => {
                let index = req[0] as usize;
                let mut seq = [Keycode::from_raw(0); MAX_LEADER_SEQ];
                for (k, kc) in seq.iter_mut().enumerate() {
                    *kc = kc_le(req, 2 + k * 2);
                }
                let cfg = LeaderCfg {
                    seq,
                    len: req[1],
                    action: kc_le(req, 2 + MAX_LEADER_SEQ * 2),
                };
                if leader_set(index, cfg) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_LEADER_GET => {
                let index = req[0] as usize;
                match leader_get(index) {
                    Some(cfg) => {
                        pack_leader(cfg, out);
                        Status::Ok
                    }
                    None => Status::BadArg,
                }
            }
            // Timed is the registry's last feature, so it is the catch-all for both
            // groups routed through `run_on_kcp`: an unrecognised op in the macro or
            // behaviour group is BadCmd — a known group, unknown op — not the
            // Unsupported an unknown *group* gets. SOCD/override ops are already
            // claimed by the earlier features, and any other group is not ours.
            _ if matches!(cmd >> 4, kcp::group::MACRO | kcp::group::BEHAVIOR) => Status::BadCmd,
            _ => return None,
        };
        Some(status)
    }

    fn on_save(&self, out: &mut [u8]) {
        config::save_feature(self.id(), out);
    }

    fn on_load(&self, buf: &[u8]) {
        config::load_feature(self.id(), buf);
    }
}

/// Read a little-endian keycode from `payload[off..off+2]`.
fn kc_le(payload: &[u8], off: usize) -> Keycode {
    Keycode::from_raw(u16::from_le_bytes([payload[off], payload[off + 1]]))
}

/// Pack a tap-dance entry for [`CMD_TAPDANCE_GET`](crate::kcp::CMD_TAPDANCE_GET):
/// `present` then `[tap, hold, double, term]` (each little-endian). An empty slot
/// writes `present = 0`.
fn pack_tapdance(cfg: Option<TapDanceCfg>, out: &mut [u8]) {
    match cfg {
        Some(c) => {
            out[0] = 1;
            out[1..3].copy_from_slice(&c.tap.raw().to_le_bytes());
            out[3..5].copy_from_slice(&c.hold.raw().to_le_bytes());
            out[5..7].copy_from_slice(&c.double.raw().to_le_bytes());
            out[7..9].copy_from_slice(&c.tap_term_ms.to_le_bytes());
        }
        None => out[0] = 0,
    }
}

/// Pack a combo for [`CMD_COMBO_GET`](crate::kcp::CMD_COMBO_GET): `present`, `len`,
/// the four member keycodes (little-endian; unused slots zero), the action keycode,
/// the term and the per-combo flags byte. An empty slot writes `present = 0`.
fn pack_combo(cfg: Option<ComboCfg>, out: &mut [u8]) {
    match cfg {
        Some(c) => {
            out[0] = 1;
            out[1] = c.len;
            for (i, k) in c.keys.iter().enumerate() {
                out[2 + i * 2..4 + i * 2].copy_from_slice(&k.raw().to_le_bytes());
            }
            out[10..12].copy_from_slice(&c.action.raw().to_le_bytes());
            out[12..14].copy_from_slice(&c.term_ms.to_le_bytes());
            out[14] = c.flags;
        }
        None => out[0] = 0,
    }
}

/// Pack a leader entry for [`CMD_LEADER_GET`](crate::kcp::CMD_LEADER_GET): `len`,
/// the [`MAX_LEADER_SEQ`] sequence keycodes (little-endian; unused slots zero) and
/// the action keycode. An empty slot is `len = 0` with the rest of the
/// (already-zeroed) payload clear.
fn pack_leader(cfg: LeaderCfg, out: &mut [u8]) {
    out[0] = cfg.len;
    for (i, kc) in cfg.seq.iter().enumerate() {
        out[1 + i * 2..3 + i * 2].copy_from_slice(&kc.raw().to_le_bytes());
    }
    let action_off = 1 + MAX_LEADER_SEQ * 2;
    out[action_off..action_off + 2].copy_from_slice(&cfg.action.raw().to_le_bytes());
}
