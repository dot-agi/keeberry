// SPDX-License-Identifier: GPL-2.0-or-later
//! Stateless input behaviours processed while building the keyboard report.
//!
//! This module is the BEHAVIOR feature (kcp group `0x7x`). It carries the two
//! behaviours that need no timing or state engine — they are pure functions of a
//! single scan and a small RAM table — so they fold straight into the report
//! builder. Tap-dance, combos and macros are deliberately out of scope here: they
//! need a per-key timer and a press/release state machine and live in
//! [`crate::timed`].
//!
//! * **SOCD** ([`apply_socd`]) — Simultaneous-Opposing-Cardinal-Directions
//!   cleanup. When both keys of a configured pair are held it suppresses one so a
//!   game never sees, say, left *and* right at once. The decision can need press
//!   order ([`SocdMode::LastWins`] / [`SocdMode::FirstWins`]); see the press-order
//!   model on [`apply_socd`].
//! * **Key overrides** ([`apply_overrides`]) — a held trigger key plus an exact
//!   set of modifiers, on a matching layer, is rewritten to a different key and
//!   modifier set (QMK key-override semantics, simplified). See the matching rule
//!   on [`apply_overrides`].
//!
//! # Where it runs
//!
//! SOCD and key overrides are two [`Feature`]s ([`Socd`], [`Overrides`]) in the
//! registry's report fold. [`crate::keymap::compute_report`] resolves the held
//! matrix into a modifier byte and a [`KeySet`] of basic keycodes, then calls
//! [`crate::features::run_on_report`], which runs [`Socd`] (via [`apply_socd`])
//! followed by [`Overrides`] (via [`apply_overrides`]) on that working set *before*
//! finalising the HID report — the same fixed order as before, since SOCD precedes
//! overrides in [`FEATURES`](crate::features::FEATURES). Each feature gates on its
//! own [`AtomicBool`] ([`Feature::active`]) so the dispatcher skips it for a single
//! relaxed load until the host configures a table over kcp, and the `apply_*`
//! functions themselves also early-out on that flag, so an unconfigured keyboard's
//! report passes through the fold untouched (the empty-table no-op each `apply_*`
//! documents).
//!
//! # State, and why the borrows are sound
//!
//! The tables ([`SocdPair`]s, the per-pair runtime edge state, and
//! [`KeyOverride`]s) live in RAM behind blocking [`Mutex`]es with an inner
//! [`RefCell`], exactly as [`crate::keymap`]'s live keymap does. Every access —
//! the report builder reading a table each scan, or the kcp BEHAVIOR group
//! writing one — is a *synchronous* critical section: the [`RefCell`] borrow is
//! held only for the few instructions that copy the small table out or store one
//! cell, and **no `.await` is ever held across a borrow**. The reader
//! ([`crate::keymap::compute_report`], on the keyboard loop) and the writer (the
//! kcp loop) are two futures on the *same* cooperative thread-mode executor,
//! which can only switch tasks at an `.await`; because no borrow spans an
//! `.await`, a read and a write can never be live at once, so the [`RefCell`] is
//! never borrowed re-entrantly and its runtime check cannot panic. The
//! [`CriticalSectionRawMutex`] additionally locks out interrupt-context access.
//! `apply_*` copies its table out under the lock and then drops the lock before
//! it touches the [`KeySet`], so not even a borrow guard outlives the critical
//! section.
//!
//! Each table is shadowed by an [`AtomicBool`] "any active" flag, recomputed
//! under the same lock whenever the table is mutated, so the per-scan fast path
//! is a single relaxed load with no critical section at all while the feature is
//! unconfigured.
//!
//! # Persistence
//!
//! The tables are RAM-live and persisted as part of the full-config blob
//! ([`crate::config`]): a host `CONFIG.SAVE` snapshots them to flash and they are
//! restored at boot.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;

use crate::config;
use crate::features::{Ctx, Feature, FeatureId, FEATURE_ALWAYS_ON, FEATURE_DEFAULT_ON};
use crate::kcp::{self, Status};
use crate::keycode::Keycode;

/// Maximum number of SOCD pairs the RAM table holds.
pub const MAX_SOCD: usize = 8;

/// Maximum number of key overrides the RAM table holds.
pub const MAX_OVERRIDES: usize = 16;

/// Sentinel index for the kcp `SOCD_CLEAR` / `OVERRIDE_CLEAR` operations that
/// clears the whole table instead of one slot. It is outside every valid slot
/// index (`MAX_SOCD`, `MAX_OVERRIDES` are both far below `0xFF`).
pub const CLEAR_ALL: u8 = 0xFF;

/// Capacity of the per-scan [`KeySet`] working buffer.
///
/// The report itself holds only six basic keys, but the set is collected
/// *before* SOCD/override processing, so it carries headroom above six: SOCD can
/// then suppress keys back under the six-key limit (avoiding a spurious rollover
/// error) and overrides can rewrite within a larger pressed set. A set that fills
/// past this cap marks itself [`truncated`](KeySet::truncated), which the report
/// builder treats as rollover overflow — the same outcome the unbuffered builder
/// produced past six keys, so the cap never changes observable behaviour.
const KEYSET_CAP: usize = 16;

/// How a SOCD pair resolves when both of its keys are held.
///
/// The discriminants are the kcp wire values (`SOCD_SET` `mode` byte); decode an
/// untrusted byte with [`SocdMode::from_u8`].
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum SocdMode {
    /// Keep the most-recently-pressed key, suppress the other (the common
    /// "rappy snappy" / null-bind feel). Needs press order — see [`apply_socd`].
    LastWins = 0,
    /// Suppress both keys while both are held (a true neutral / SOCD "rocker").
    Neutral = 1,
    /// Keep the first-pressed key, suppress whichever was pressed later. Needs
    /// press order — see [`apply_socd`].
    FirstWins = 2,
}

impl SocdMode {
    /// Decode the kcp `mode` byte; `None` for an unassigned value (the BEHAVIOR
    /// group maps that to [`Status::BadArg`](crate::kcp::Status::BadArg)).
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(SocdMode::LastWins),
            1 => Some(SocdMode::Neutral),
            2 => Some(SocdMode::FirstWins),
            _ => None,
        }
    }
}

/// One configured SOCD pair: two opposing keys and how they resolve when both are
/// held. The pair's keys should be distinct, real keycodes (the gaming case is a
/// basic key such as `KEY_A`/`KEY_D`); this is the unit the kcp `SOCD_SET`/`GET`
/// operations carry and the CONFIG blob persists ([`crate::config`]).
#[derive(Clone, Copy)]
pub struct SocdPair {
    /// First key of the pair.
    pub a: Keycode,
    /// Second key of the pair.
    pub b: Keycode,
    /// How the pair resolves when both keys are held.
    pub mode: SocdMode,
}

/// Which key of a SOCD pair was pressed most recently, tracked across scans by
/// [`apply_socd`] for the order-sensitive modes.
#[derive(Clone, Copy)]
enum LastPressed {
    /// No rising edge has been observed (the initial / just-reset state). The
    /// order-sensitive modes fall back to keeping `a`, but the same scan that
    /// first sees both keys held also records a rising edge, so in practice this
    /// is a defensive default rather than a reachable both-held state.
    Unknown,
    /// `a` was the last to transition from released to held.
    A,
    /// `b` was the last to transition from released to held.
    B,
}

/// Per-pair runtime state for SOCD press-order tracking.
///
/// Press order cannot be read from a single scan — the matrix only says which
/// keys are *held now* — so it is reconstructed by edge detection: each scan
/// remembers whether each key was held last scan (`prev_*`) and, on a
/// released→held transition, records that key as the most recent in `last`. This
/// runs every scan the pair is configured, independent of whether both keys are
/// held, so `last` is always current the instant both become held.
#[derive(Clone, Copy)]
struct SocdRuntime {
    /// Whether `a` was held on the previous scan (for rising-edge detection).
    prev_a: bool,
    /// Whether `b` was held on the previous scan.
    prev_b: bool,
    /// Which key was pressed most recently.
    last: LastPressed,
}

impl SocdRuntime {
    /// Initial state: neither key seen held, order unknown.
    const fn new() -> Self {
        Self {
            prev_a: false,
            prev_b: false,
            last: LastPressed::Unknown,
        }
    }
}

/// One key override: a trigger key + exact modifier set, gated on a layer mask,
/// rewritten to a replacement key + modifier set. This is the unit the kcp
/// `OVERRIDE_SET`/`GET` operations carry. See [`apply_overrides`] for the
/// matching rule.
#[derive(Clone, Copy)]
pub struct KeyOverride {
    /// Key that must be held for the override to fire.
    pub trigger: Keycode,
    /// Modifier byte that must be held *exactly* (HID modifier bits, `0xE0..0xE7`
    /// as bits `0..7`) for the override to fire.
    pub trigger_mods: u8,
    /// Key substituted for [`trigger`](Self::trigger) when the override fires. A
    /// modifier folds into the report modifier byte; `NONE` simply drops the
    /// trigger (a "disable this combo" override).
    pub replacement: Keycode,
    /// Modifier byte the report carries when the override fires (replaces the
    /// held modifiers).
    pub replacement_mods: u8,
    /// Layers on which the override is active (bit `n` = layer `n`); it fires only
    /// when this intersects the scan's active-layer mask.
    pub layer_mask: u16,
    /// Whether the override is active. A disabled slot is stored but never fires
    /// and never trips the fast-path flag.
    pub enabled: bool,
}

/// The set of resolved basic keycodes for one scan, before it is finalised into
/// the six-slot HID report.
///
/// [`crate::keymap::compute_report`] fills this with every held key that resolves
/// to a basic usage (modifiers fold into a separate byte; consumer/no-op/layer
/// keys contribute nothing), then the behaviours rewrite it in place: SOCD
/// suppresses a key by replacing it with [`Keycode::NO`], an override swaps a
/// trigger for its replacement. The builder then walks [`as_slice`](Self::as_slice)
/// and emits each entry's usage; a `NO` entry classifies as no-op and emits
/// nothing, which is exactly how a suppressed key disappears from the report.
///
/// It is [`Copy`] so [`apply_overrides`] can snapshot the post-SOCD set once and
/// match every override against that physical set rather than the set it is
/// mutating — without the snapshot, an override that creates a key could let a
/// later override fire from a key no physical press produced (A->B->C chaining).
#[derive(Clone, Copy)]
pub struct KeySet {
    keys: [Keycode; KEYSET_CAP],
    len: usize,
    truncated: bool,
}

impl KeySet {
    /// An empty set.
    pub const fn new() -> Self {
        Self {
            keys: [Keycode::NO; KEYSET_CAP],
            len: 0,
            truncated: false,
        }
    }

    /// Append a resolved keycode. Past [`KEYSET_CAP`] the key is dropped and the
    /// set is flagged [`truncated`](Self::truncated); the report builder maps that
    /// to a rollover-overflow report, matching the >6-key behaviour.
    pub fn push(&mut self, kc: Keycode) {
        if self.len < KEYSET_CAP {
            self.keys[self.len] = kc;
            self.len += 1;
        } else {
            self.truncated = true;
        }
    }

    /// Whether more keys were pushed than the buffer holds.
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    /// The collected keys, in scan order, for the report builder to emit.
    pub fn as_slice(&self) -> &[Keycode] {
        &self.keys[..self.len]
    }

    /// Whether `kc` is currently present in the set.
    fn contains(&self, kc: Keycode) -> bool {
        self.keys[..self.len].iter().any(|&k| k == kc)
    }

    /// Suppress `kc`: replace every occurrence with [`Keycode::NO`] so the report
    /// builder emits nothing for it.
    fn suppress(&mut self, kc: Keycode) {
        for k in self.keys[..self.len].iter_mut() {
            if *k == kc {
                *k = Keycode::NO;
            }
        }
    }

    /// Replace every occurrence of `from` with `to` (a key override's swap).
    fn replace(&mut self, from: Keycode, to: Keycode) {
        for k in self.keys[..self.len].iter_mut() {
            if *k == from {
                *k = to;
            }
        }
    }

    /// Replace the keycode at `index` (a no-op if out of range or already dropped by
    /// truncation). Used to settle a placeholder — the grave-escape slot — to its final
    /// keycode after the report fold, rewriting only that one slot.
    pub fn replace_at(&mut self, index: usize, kc: Keycode) {
        if index < self.len {
            self.keys[index] = kc;
        }
    }
}

// === SOCD table ============================================================

/// Configured SOCD pairs. Default empty; mutated only by the kcp BEHAVIOR group.
static SOCD: Mutex<CriticalSectionRawMutex, RefCell<[Option<SocdPair>; MAX_SOCD]>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new([None; MAX_SOCD]));

/// Per-pair runtime edge state, parallel to [`SOCD`]. Reset whenever a slot is
/// (re)configured or cleared so stale press order never leaks across a rebind.
static SOCD_RT: Mutex<CriticalSectionRawMutex, RefCell<[SocdRuntime; MAX_SOCD]>> = Mutex::const_new(
    CriticalSectionRawMutex::new(),
    RefCell::new([SocdRuntime::new(); MAX_SOCD]),
);

/// Fast-path flag: any [`SOCD`] slot configured? Recomputed under the lock on
/// every mutation so [`apply_socd`] can early-out with one relaxed load.
static SOCD_ANY: AtomicBool = AtomicBool::new(false);

// === Override table ========================================================

/// Configured key overrides. Default empty; mutated only by the kcp BEHAVIOR
/// group.
static OVERRIDES: Mutex<CriticalSectionRawMutex, RefCell<[Option<KeyOverride>; MAX_OVERRIDES]>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new([None; MAX_OVERRIDES]));

/// Fast-path flag: any *enabled* [`OVERRIDES`] slot configured? Recomputed under
/// the lock on every mutation.
static OVERRIDE_ANY: AtomicBool = AtomicBool::new(false);

/// Apply SOCD cleanup to the resolved key set for one scan.
///
/// For every configured pair this first updates its press-order state, then, if
/// both keys are held, suppresses one according to the pair's [`SocdMode`].
///
/// # Press-order model
///
/// The matrix exposes only "held now", so press order is reconstructed by edge
/// detection across scans ([`SocdRuntime`]). Each scan, for each pair: `a`/`b`
/// presence is read from the set; a released→held transition on a key records it
/// as `last` (if both rise in the same scan, `b` is taken as the most recent,
/// since it is recorded second — a deterministic tiebreak for a sub-scan order
/// the hardware cannot distinguish); then `prev_a`/`prev_b` are stored for the
/// next scan. Because this runs every scan — not only when both are held — `last`
/// is already correct the instant both keys become held. The resolution, when
/// both are held:
///
/// * [`Neutral`](SocdMode::Neutral) — suppress both.
/// * [`LastWins`](SocdMode::LastWins) — keep `last`, suppress the other.
/// * [`FirstWins`](SocdMode::FirstWins) — suppress `last`, keep the other.
///
/// The [`Unknown`](LastPressed::Unknown) fallback (keep `a`) is defensive: when a
/// pair is (re)configured while both keys are already held, the first scan that
/// sees them held also sees both as rising edges and resolves the tie to `b`
/// (recorded second), so `last` is already `A` or `B` whenever it is read with
/// both keys down. The fallback therefore does not arise in normal operation.
///
/// Pairs are processed in index order and should use disjoint keys; presence is
/// read from the set as it is progressively suppressed, so overlapping pairs
/// would interact in index order.
///
/// This is a no-op (one relaxed load) until a pair is configured, so it does not
/// alter the report of an unconfigured keyboard.
fn apply_socd(keys: &mut KeySet) {
    if !SOCD_ANY.load(Ordering::Relaxed) {
        return;
    }

    // Copy both small tables out, then drop their locks before touching the key
    // set; the runtime is written back at the end. No lock is held across the
    // key-set edits, and none across any `.await` (there are none — this is a
    // synchronous call on the keyboard loop).
    let pairs = SOCD.lock(|cell| *cell.borrow());
    let mut rt = SOCD_RT.lock(|cell| *cell.borrow());

    for (slot, pair) in pairs.iter().enumerate() {
        let Some(pair) = pair else { continue };
        let st = &mut rt[slot];

        let a_now = keys.contains(pair.a);
        let b_now = keys.contains(pair.b);

        // Rising-edge press-order tracking. `b` second => `b` wins a same-scan tie.
        if a_now && !st.prev_a {
            st.last = LastPressed::A;
        }
        if b_now && !st.prev_b {
            st.last = LastPressed::B;
        }
        st.prev_a = a_now;
        st.prev_b = b_now;

        if a_now && b_now {
            match pair.mode {
                SocdMode::Neutral => {
                    keys.suppress(pair.a);
                    keys.suppress(pair.b);
                }
                // Keep the most recent; default to keeping `a` if unknown.
                SocdMode::LastWins => match st.last {
                    LastPressed::B => keys.suppress(pair.a),
                    LastPressed::A | LastPressed::Unknown => keys.suppress(pair.b),
                },
                // Suppress the most recent; default to keeping `a` if unknown.
                SocdMode::FirstWins => match st.last {
                    LastPressed::A => keys.suppress(pair.a),
                    LastPressed::B | LastPressed::Unknown => keys.suppress(pair.b),
                },
            }
        }
    }

    SOCD_RT.lock(|cell| *cell.borrow_mut() = rt);
}

/// Apply key overrides to the resolved modifier byte and key set for one scan.
///
/// # Matching rule
///
/// Two things are snapshotted once on entry so overrides cannot chain off one
/// another: `held`, the resolved modifier byte, and `pressed`, the post-SOCD key
/// set. Every override matches its `trigger_mods` against `held` and its
/// `trigger` against `pressed` — never against a modifier or key an *earlier*
/// override produced — so a single physical press can only satisfy the overrides
/// whose trigger it physically holds (no A->B->C chaining). For each enabled
/// override, in index order, it fires when **all** of:
///
/// * its `layer_mask` intersects `active_layers` (the scan's active layers);
/// * its `trigger_mods` equals `held` *exactly* (not a subset — the simplified
///   QMK exact-trigger-mod match); and
/// * its `trigger` key is present in the `pressed` snapshot.
///
/// On firing, the trigger key is replaced by `replacement` in the *live* set and
/// the report modifier byte is set to `replacement_mods`. A `replacement` that is
/// a modifier folds into the modifier byte when the report is finalised; `NONE`
/// drops the trigger entirely. If several overrides fire in one scan (all
/// matching the same `held`/`pressed` basis), each swaps its own trigger in the
/// live set and the modifier byte takes the `replacement_mods` of the last one in
/// index order.
///
/// This is a no-op (one relaxed load) until an enabled override is configured.
fn apply_overrides(report_mods: &mut u8, keys: &mut KeySet, active_layers: u16) {
    if !OVERRIDE_ANY.load(Ordering::Relaxed) {
        return;
    }

    // Snapshot the match basis once: the held modifier byte and the post-SOCD key
    // set. Overrides match against these, but apply their swaps to the live
    // `keys`, so an override never fires on a key/modifier another override made.
    let held = *report_mods;
    let pressed = *keys;
    // Copy the table out and drop the lock before editing the key set.
    let table = OVERRIDES.lock(|cell| *cell.borrow());

    for entry in table.iter() {
        let Some(ov) = entry else { continue };
        if !ov.enabled
            || ov.layer_mask & active_layers == 0
            || ov.trigger_mods != held
            || !pressed.contains(ov.trigger)
        {
            continue;
        }
        keys.replace(ov.trigger, ov.replacement);
        *report_mods = ov.replacement_mods;
    }
}

// === kcp BEHAVIOR group accessors ==========================================

/// Recompute [`SOCD_ANY`] from the live table (called under no lock; takes its
/// own).
fn recompute_socd_any() {
    let any = SOCD.lock(|cell| cell.borrow().iter().any(Option::is_some));
    SOCD_ANY.store(any, Ordering::Relaxed);
}

/// Recompute [`OVERRIDE_ANY`] (true only if some slot is configured *and*
/// enabled).
fn recompute_override_any() {
    let any = OVERRIDES.lock(|cell| {
        cell.borrow()
            .iter()
            .any(|entry| matches!(entry, Some(ov) if ov.enabled))
    });
    OVERRIDE_ANY.store(any, Ordering::Relaxed);
}

/// Configure SOCD slot `index`. Resets that slot's press-order runtime. Returns
/// `false` (writing nothing) when `index` is out of range.
pub fn socd_set(index: usize, a: Keycode, b: Keycode, mode: SocdMode) -> bool {
    if index >= MAX_SOCD {
        return false;
    }
    SOCD.lock(|cell| cell.borrow_mut()[index] = Some(SocdPair { a, b, mode }));
    SOCD_RT.lock(|cell| cell.borrow_mut()[index] = SocdRuntime::new());
    recompute_socd_any();
    true
}

/// Clear SOCD slot `index`. Returns `false` when `index` is out of range.
pub fn socd_clear(index: usize) -> bool {
    if index >= MAX_SOCD {
        return false;
    }
    SOCD.lock(|cell| cell.borrow_mut()[index] = None);
    SOCD_RT.lock(|cell| cell.borrow_mut()[index] = SocdRuntime::new());
    recompute_socd_any();
    true
}

/// Clear every SOCD slot.
pub fn socd_clear_all() {
    SOCD.lock(|cell| *cell.borrow_mut() = [None; MAX_SOCD]);
    SOCD_RT.lock(|cell| *cell.borrow_mut() = [SocdRuntime::new(); MAX_SOCD]);
    SOCD_ANY.store(false, Ordering::Relaxed);
}

/// Read SOCD slot `index`. `None` for an empty slot or an out-of-range index.
pub fn socd_get(index: usize) -> Option<SocdPair> {
    if index >= MAX_SOCD {
        return None;
    }
    SOCD.lock(|cell| cell.borrow()[index])
}

/// Configure override slot `index`. Returns `false` when `index` is out of range.
pub fn override_set(index: usize, ov: KeyOverride) -> bool {
    if index >= MAX_OVERRIDES {
        return false;
    }
    OVERRIDES.lock(|cell| cell.borrow_mut()[index] = Some(ov));
    recompute_override_any();
    true
}

/// Clear override slot `index`. Returns `false` when `index` is out of range.
pub fn override_clear(index: usize) -> bool {
    if index >= MAX_OVERRIDES {
        return false;
    }
    OVERRIDES.lock(|cell| cell.borrow_mut()[index] = None);
    recompute_override_any();
    true
}

/// Clear every override slot.
pub fn override_clear_all() {
    OVERRIDES.lock(|cell| *cell.borrow_mut() = [None; MAX_OVERRIDES]);
    OVERRIDE_ANY.store(false, Ordering::Relaxed);
}

/// Read override slot `index`. `None` for an empty slot or an out-of-range index.
pub fn override_get(index: usize) -> Option<KeyOverride> {
    if index >= MAX_OVERRIDES {
        return None;
    }
    OVERRIDES.lock(|cell| cell.borrow()[index])
}

// === Feature impls =========================================================
//
// SOCD and key overrides are two registry features. Their hooks delegate to the
// `apply_*`/accessor functions above (which own the tables, locks and the ANY
// gates), so the registry is purely the call seam: `active` mirrors the table's
// ANY flag, `on_report` runs the matching `apply_*`, `on_kcp` owns the
// behaviour-group opcodes (the SOCD-pair and key-override get/set ops), and
// `on_save`/`on_load` defer to the fixed-offset blob logic in [`crate::config`].

/// SOCD-cleanup feature ([`apply_socd`]).
pub struct Socd;

/// Key-override feature ([`apply_overrides`]).
pub struct Overrides;

impl Feature for Socd {
    fn id(&self) -> FeatureId {
        FeatureId::Socd
    }

    fn name(&self) -> &'static str {
        "SOCD Cleanup"
    }

    /// Always-on: SOCD cleanup is part of the report-resolution pipeline (it runs
    /// before key overrides, which match the post-SOCD set), so it is structural
    /// rather than a user-toggleable add-on.
    fn flags(&self) -> u8 {
        FEATURE_DEFAULT_ON | FEATURE_ALWAYS_ON
    }

    /// Mirrors the [`SOCD_ANY`] fast-path flag, so the dispatcher skips the report
    /// fold for a single relaxed load until a pair is configured.
    fn active(&self) -> bool {
        SOCD_ANY.load(Ordering::Relaxed)
    }

    fn on_report(&self, _c: &Ctx, _mods: &mut u8, keys: &mut KeySet) {
        apply_socd(keys);
    }

    fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
        let status = match cmd {
            kcp::CMD_SOCD_SET => {
                let index = req[0] as usize;
                let a = Keycode::from_raw(u16::from_le_bytes([req[1], req[2]]));
                let b = Keycode::from_raw(u16::from_le_bytes([req[3], req[4]]));
                match SocdMode::from_u8(req[5]) {
                    Some(mode) if socd_set(index, a, b, mode) => Status::Ok,
                    _ => Status::BadArg,
                }
            }
            kcp::CMD_SOCD_CLEAR => {
                let index = req[0];
                if index == CLEAR_ALL {
                    socd_clear_all();
                    Status::Ok
                } else if socd_clear(index as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_SOCD_GET => {
                let index = req[0] as usize;
                if index >= MAX_SOCD {
                    Status::BadArg
                } else {
                    pack_socd_pair(socd_get(index), out);
                    Status::Ok
                }
            }
            // The behaviour-group capacity reply spans both tables; SOCD owns it as
            // the group's first feature.
            kcp::CMD_BEHAVIOR_INFO => {
                out[0] = MAX_SOCD as u8;
                out[1] = MAX_OVERRIDES as u8;
                Status::Ok
            }
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

impl Feature for Overrides {
    fn id(&self) -> FeatureId {
        FeatureId::Overrides
    }

    fn name(&self) -> &'static str {
        "Key Overrides"
    }

    /// Always-on: like SOCD, key overrides are part of the structural report-
    /// resolution pipeline, not a user-toggleable add-on.
    fn flags(&self) -> u8 {
        FEATURE_DEFAULT_ON | FEATURE_ALWAYS_ON
    }

    /// Mirrors the [`OVERRIDE_ANY`] fast-path flag (true only when some slot is
    /// configured *and* enabled).
    fn active(&self) -> bool {
        OVERRIDE_ANY.load(Ordering::Relaxed)
    }

    fn on_report(&self, c: &Ctx, mods: &mut u8, keys: &mut KeySet) {
        apply_overrides(mods, keys, c.active_layers);
    }

    fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
        let status = match cmd {
            kcp::CMD_OVERRIDE_SET => {
                let index = req[0] as usize;
                let ov = KeyOverride {
                    trigger: Keycode::from_raw(u16::from_le_bytes([req[1], req[2]])),
                    trigger_mods: req[3],
                    replacement: Keycode::from_raw(u16::from_le_bytes([req[4], req[5]])),
                    replacement_mods: req[6],
                    layer_mask: u16::from_le_bytes([req[7], req[8]]),
                    enabled: req[9] != 0,
                };
                if override_set(index, ov) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_OVERRIDE_CLEAR => {
                let index = req[0];
                if index == CLEAR_ALL {
                    override_clear_all();
                    Status::Ok
                } else if override_clear(index as usize) {
                    Status::Ok
                } else {
                    Status::BadArg
                }
            }
            kcp::CMD_OVERRIDE_GET => {
                let index = req[0] as usize;
                if index >= MAX_OVERRIDES {
                    Status::BadArg
                } else {
                    pack_override(override_get(index), out);
                    Status::Ok
                }
            }
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

/// Pack a SOCD slot for [`CMD_SOCD_GET`](crate::kcp::CMD_SOCD_GET) into the reply
/// payload: `present` byte then `[a_lo, a_hi, b_lo, b_hi, mode]`. An empty slot
/// writes `present = 0` and leaves the rest of the (already-zeroed) payload clear.
fn pack_socd_pair(pair: Option<SocdPair>, out: &mut [u8]) {
    match pair {
        Some(p) => {
            out[0] = 1;
            out[1..3].copy_from_slice(&p.a.raw().to_le_bytes());
            out[3..5].copy_from_slice(&p.b.raw().to_le_bytes());
            out[5] = p.mode as u8;
        }
        None => out[0] = 0,
    }
}

/// Pack an override slot for [`CMD_OVERRIDE_GET`](crate::kcp::CMD_OVERRIDE_GET)
/// into the reply payload: `present` byte then `[trig_lo, trig_hi, trig_mods,
/// repl_lo, repl_hi, repl_mods, layer_lo, layer_hi, enabled]`. An empty slot
/// writes `present = 0`.
fn pack_override(ov: Option<KeyOverride>, out: &mut [u8]) {
    match ov {
        Some(o) => {
            out[0] = 1;
            out[1..3].copy_from_slice(&o.trigger.raw().to_le_bytes());
            out[3] = o.trigger_mods;
            out[4..6].copy_from_slice(&o.replacement.raw().to_le_bytes());
            out[6] = o.replacement_mods;
            out[7..9].copy_from_slice(&o.layer_mask.to_le_bytes());
            out[9] = o.enabled as u8;
        }
        None => out[0] = 0,
    }
}
