// SPDX-License-Identifier: GPL-2.0-or-later
//! Compile-time feature registry: the trait/seam every cross-cutting behaviour
//! plugs into.
//!
//! keeberry's behaviours already share one shape — a RAM table behind a blocking
//! [`Mutex`](embassy_sync::blocking_mutex::Mutex), an `AtomicBool` "any active"
//! gate, kcp accessors and a serialize/deserialize in [`crate::config`]. This
//! module *names* that shape as a [`Feature`] trait and collects every feature in
//! one ordered [`FEATURES`] array, so the keyboard loop, the kcp dispatcher and the
//! config blob route through a single seam instead of hard-coding each behaviour's
//! call. A new behaviour becomes one `impl Feature` plus one array entry; the call
//! sites never change again.
//!
//! # Dispatch semantics (array order = priority)
//!
//! The order in [`FEATURES`] is the fixed, compile-time priority order (the same
//! discipline QMK's community-module chain uses). Each hook has one dispatch fn:
//!
//! * **fold** ([`run_on_matrix`], [`run_on_report`], [`run_on_overlay`]) — every
//!   active feature transforms the value in turn, in array order.
//! * **first-claims** ([`run_on_kcp`]) — the first feature that returns
//!   [`Some`] handles the request; the rest are skipped.
//! * **sequence** ([`run_on_tick`], [`run_on_save`], [`run_on_load`]) — every
//!   feature runs.
//!
//! # Runtime enable/disable
//!
//! Every feature also has a persisted master switch, held centrally in one
//! [`ENABLED`] bitmap (one bit per [`FeatureId`]) rather than per feature, so the
//! [`FEATURES` kcp group](features_dispatch) can enumerate and toggle the whole
//! registry with no per-feature wiring and [`crate::config`] persists the switches
//! in a single word. The per-scan hooks skip a feature whose bit is clear, so a
//! disabled feature is wholly inert; [`Feature::on_disable`] clears its transient
//! state on the off edge so a re-enable starts clean. Structural features
//! ([`FEATURE_ALWAYS_ON`]) cannot be switched off. Every feature ships enabled
//! ([`FEATURE_DEFAULT_ON`]), so the default build behaves exactly as before.
//!
//! # The 1 kHz budget
//!
//! The per-scan hooks ([`run_on_matrix`]/[`run_on_report`]/[`run_on_overlay`]/
//! [`run_on_tick`]) gate each feature on its enable bit *and* [`Feature::active`],
//! so an idle feature costs one relaxed bitmap load plus its `AtomicBool` ANY load
//! and nothing else — preserving today's fast path. The persistence and kcp hooks
//! are *not* gated: a config restore must rebuild a feature's table even though it
//! is idle until it does, and a kcp request to configure a feature must be served
//! while it is still disabled (so the GUI can set it up before switching it on).
//!
//! # Soundness
//!
//! Every hook is synchronous — no `.await` is ever held across a feature's
//! internal `RefCell` borrow — so the cooperative executor cannot interleave a
//! reader and a writer of the same table mid-borrow, exactly as the individual
//! behaviour modules already document.

use core::sync::atomic::{AtomicU32, Ordering};

use embassy_time::Instant;

// The `feature!` declarative macro (in this module's `macros.rs`) fuses each plugin's
// struct + singleton + `id`/`name`/`flags` boilerplate into one declaration; `#[macro_use]`
// brings it into scope for the plugin modules below. It deliberately does not register
// anything: the `FEATURES` array below stays explicit because its order is priority.
#[macro_use]
mod macros;

// The behaviour plugins. Each is a `#[cfg]`-gated module + one `FEATURES` entry;
// disabling its Cargo feature drops the module, the registry entry and all its flash.
#[cfg(feature = "autocorrect")]
pub mod autocorrect;
#[cfg(feature = "caps_word")]
pub mod caps_word;
#[cfg(feature = "key_lock")]
pub mod key_lock;
#[cfg(feature = "one_shot_mod")]
pub mod one_shot_mod;
#[cfg(feature = "repeat_key")]
pub mod repeat_key;
#[cfg(feature = "unicode")]
pub mod unicode;
// @scaffold:features-mod — `just new-feature <Name>` inserts a new plugin's
// `#[cfg(feature = "<name>")] pub mod <name>;` declaration at this point.

use crate::behavior::KeySet;
use crate::kcp::Status;
use crate::keymap::Report;
use crate::matrix::NUM_ROWS;
use crate::rgb::{Rgb, RgbCtx, LED_COUNT};

/// Whether HID usage `u` is a basic letter (`a`–`z`, usages `0x04..=0x1D`). The
/// shared predicate behind Caps Word's per-letter Shift and Autocorrect's word-buffer
/// match, so both read the letter range from one definition. Gated on those two
/// features, since they are its only callers.
#[cfg(any(feature = "caps_word", feature = "autocorrect"))]
pub(crate) const fn is_hid_letter(u: u8) -> bool {
    matches!(u, 0x04..=0x1D)
}

/// Read-only per-scan context shared by the fold hooks, so each hook signature
/// carries only its mutable target plus this.
///
/// It holds exactly what today's behaviours read: `active_layers` (key overrides
/// gate on the active layer mask), `prev_matrix` and `now` (the timed engine
/// resolves edges against last scan's matrix and times its state machine off the
/// scan timestamp). The scan's matrix itself is not carried because the only hook
/// that needs it — [`Feature::on_matrix`] — receives it as its `&mut` target.
#[derive(Clone, Copy)]
pub struct Ctx<'a> {
    /// This scan's timestamp (the keyboard loop's `t0`).
    pub now: Instant,
    /// Active-layer bitmask for the relevant resolution (bit `n` = layer `n`).
    pub active_layers: u16,
    /// Previous scan's debounced matrix — the edge basis for the matrix hook.
    pub prev_matrix: &'a [u16; NUM_ROWS],
}

/// Stable identity of each feature. Distinguishes one feature from another for any
/// keyed dispatch, and — since runtime enable/disable landed — the **wire id** the
/// [`FEATURES` kcp group](features_dispatch) reports and the bit position each
/// feature occupies in the persisted [`ENABLED`] bitmap. The discriminants are
/// therefore a stable wire/persistence format: keep them contiguous from `0` and
/// never renumber a shipped one (a new feature takes the next free value). Config
/// `save_feature`/`load_feature` match by variant, not number, so they are
/// unaffected by the exact values.
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FeatureId {
    /// SOCD cleanup ([`crate::behavior`]).
    Socd = 0,
    /// Key overrides ([`crate::behavior`]).
    Overrides = 1,
    /// The timed engine — tap-dance, combos, dynamic macros, leader, auto-shift,
    /// mod-tap / layer-tap ([`crate::timed`]).
    Timed = 2,
    /// Caps Word (the `caps_word` feature).
    CapsWord = 3,
    /// Key Lock (the `key_lock` feature).
    KeyLock = 4,
    /// Repeat Key (the `repeat_key` feature).
    RepeatKey = 5,
    /// One-Shot-Mod (the `one_shot_mod` feature).
    OneShotMod = 6,
    /// Autocorrect (the `autocorrect` feature).
    Autocorrect = 7,
    /// Unicode input (the `unicode` feature).
    Unicode = 8,
    // @scaffold:feature-id — `just new-feature <Name>` inserts the next contiguous
    // `<Name> = <n>,` discriminant here (the stable wire/persist id; never renumber a
    // shipped one) and bumps the `< 32` enable-bitmap guard below.
}

/// [`Feature::flags`] bit: the feature ships enabled at the factory (the default).
/// A reset and a config blob without the feature's bit set both fall back to this.
pub const FEATURE_DEFAULT_ON: u8 = 1 << 0;

/// [`Feature::flags`] bit: the feature is structural and cannot be switched off at
/// runtime — [`set_enabled`] rejects a disable with [`Status::BadArg`] and the
/// enable bit is force-held on through a config restore. Carried by the always-on
/// behaviour core (SOCD, key overrides, the timed engine), whose removal would
/// strand the keycodes and report-pipeline invariants the keymap is built on.
pub const FEATURE_ALWAYS_ON: u8 = 1 << 1;

/// A compile-time feature. Every hook defaults to a no-op, so a feature overrides
/// only the hooks it uses. `Sync` because [`FEATURES`] is a shared `&'static`
/// array of trait objects.
///
/// The hook set maps one-to-one onto the existing call sites; see the module
/// docs for each hook's dispatch semantics.
pub trait Feature: Sync {
    /// This feature's stable identity.
    fn id(&self) -> FeatureId;

    /// Human label for the GUI's auto-rendered Features panel, reported over the
    /// [`FEATURES` kcp group](features_dispatch). A `&'static str` — a ptr+len into
    /// flash, so it costs only its bytes.
    fn name(&self) -> &'static str;

    /// Capability flags ([`FEATURE_DEFAULT_ON`] | [`FEATURE_ALWAYS_ON`]). Defaults
    /// to a plain default-on, user-toggleable feature; the structural core overrides
    /// it to add [`FEATURE_ALWAYS_ON`].
    fn flags(&self) -> u8 {
        FEATURE_DEFAULT_ON
    }

    /// O(1) "is anything configured?" gate — the per-feature `AtomicBool`-ANY
    /// load. Returns `true` by default; a feature with idle state overrides it so
    /// the per-scan hooks skip it for a single relaxed load.
    fn active(&self) -> bool {
        true
    }

    /// Clear any transient state when the feature is switched off ([`set_enabled`]
    /// calls this on the on→off edge), so a disable mid-action cannot strand state
    /// and a later re-enable starts clean. Default no-op — a stateless or always-on
    /// feature needs nothing here.
    fn on_disable(&self) {}

    /// Transform the effective matrix (fold): suppress claimed positions before
    /// the report builder resolves them.
    fn on_matrix(&self, _c: &Ctx, _m: &mut [u16; NUM_ROWS]) {}

    /// Rewrite the resolved modifier byte and basic-key set (fold) while the
    /// report is built.
    fn on_report(&self, _c: &Ctx, _mods: &mut u8, _keys: &mut KeySet) {}

    /// Merge synthesised output into the finished report (fold), e.g. an injected
    /// tap-dance/combo/macro key.
    fn on_overlay(&self, _c: &Ctx, _r: &mut Report) {}

    /// Paint over the rendered RGB frame (first-claims): returns whether this
    /// feature claimed the frame. The base per-key effects are a separate registry
    /// ([`crate::rgb::RGB_EFFECTS`]); this hook is reserved for a feature that
    /// overlays status, and so has no dispatcher until one does.
    fn on_rgb_frame(&self, _c: &RgbCtx, _leds: &mut [Rgb; LED_COUNT]) -> bool {
        false
    }

    /// Handle a kcp request (first-claims): `Some(status)` claims `cmd`, `None`
    /// passes it to the next feature.
    fn on_kcp(&self, _cmd: u8, _req: &[u8], _out: &mut [u8]) -> Option<Status> {
        None
    }

    /// Periodic per-scan work (sequence), for timeout-driven state with no matrix
    /// edge to ride.
    fn on_tick(&self, _now: Instant) {}

    /// Snapshot this feature's RAM state into the config blob at its fixed offset.
    fn on_save(&self, _out: &mut [u8]) {}

    /// Restore this feature's RAM state from the config blob.
    fn on_load(&self, _buf: &[u8]) {}
}

/// The feature registry. Array order is priority order. A new feature is one entry
/// here plus its `impl Feature`; a `#[cfg]`-disabled feature is simply absent and
/// costs zero flash.
///
/// SOCD precedes overrides because the report fold must run SOCD cleanup before
/// key overrides (overrides match against the post-SOCD set), exactly as the
/// former hard-coded `apply_socd` → `apply_overrides` order did.
/// Repeat precedes Caps Word so a `Repeat`-injected key is itself shifted by an active
/// caps-word (the report fold runs them in this order); One-Shot-Mod runs last so its
/// modifier folds over the fully-resolved set, and Key Lock follows the timed engine so a
/// latched position is re-asserted after combo / leader suppression. Unicode follows the
/// timed engine too: its overlay injects a synthesised keystroke stream like a macro, and
/// it touches no report fold, so its position relative to the fold features is immaterial.
/// Autocorrect is last of all: while a correction plays its overlay rewrites the whole
/// report, so it must have the final say over every other feature's synthesised output.
pub static FEATURES: &[&dyn Feature] = &[
    &crate::behavior::Socd,
    &crate::behavior::Overrides,
    &crate::timed::Timed,
    #[cfg(feature = "unicode")]
    &unicode::UNICODE,
    #[cfg(feature = "repeat_key")]
    &repeat_key::REPEAT,
    #[cfg(feature = "caps_word")]
    &caps_word::CAPS_WORD,
    #[cfg(feature = "key_lock")]
    &key_lock::KEY_LOCK,
    #[cfg(feature = "one_shot_mod")]
    &one_shot_mod::ONE_SHOT_MOD,
    #[cfg(feature = "autocorrect")]
    &autocorrect::AUTOCORRECT,
    // @scaffold:features-registry — `just new-feature <Name>` inserts a new
    // `#[cfg(feature = "<name>")] &<name>::<SINGLETON>,` entry here. Its position is
    // dispatch priority (see this array's doc above), so place it deliberately — a new
    // feature is not necessarily last.
];

// The enable bitmap is a `u32`, so every `FeatureId` occupies bit `id < 32`. A 33rd
// feature (the highest discriminant reaching 32) must widen `ENABLED` here and
// `config::ENABLE_BYTES` past a word — caught at build time, never a silent shift
// overflow. Bump this guard's id when adding a feature past `Unicode`.
const _: () = assert!(
    (FeatureId::Unicode as u32) < 32,
    "enable bitmap is a u32; widen ENABLED and config::ENABLE_BYTES past 32 features"
);

/// The runtime enable bitmap: bit `FeatureId as u32` is set when that feature is
/// switched on. Populated at boot from the factory defaults ([`init_enabled`]) and
/// then overwritten by a valid config blob ([`set_enabled_map`]); the per-scan
/// dispatch reads it through [`is_enabled`]. One shared word so the whole registry's
/// switches enumerate, toggle and persist together with no per-feature wiring.
static ENABLED: AtomicU32 = AtomicU32::new(0);

/// The bit `id` occupies in [`ENABLED`].
const fn enable_bit(id: FeatureId) -> u32 {
    1 << (id as u32)
}

/// Whether feature `id` is currently switched on — the per-scan dispatch gate, one
/// relaxed load.
pub fn is_enabled(id: FeatureId) -> bool {
    ENABLED.load(Ordering::Relaxed) & enable_bit(id) != 0
}

/// Initialise [`ENABLED`] to the factory defaults: every registered feature whose
/// [`flags`](Feature::flags) request [`FEATURE_DEFAULT_ON`] or [`FEATURE_ALWAYS_ON`]
/// starts on. Called at boot before the saved config is read (so the no-saved-config
/// path leaves every feature at its default) and by a config reset.
pub fn init_enabled() {
    let mut map = 0;
    for f in FEATURES {
        if f.flags() & (FEATURE_DEFAULT_ON | FEATURE_ALWAYS_ON) != 0 {
            map |= enable_bit(f.id());
        }
    }
    ENABLED.store(map, Ordering::Relaxed);
}

/// The whole enable bitmap, for [`crate::config`] to persist in one word.
pub fn enabled_map() -> u32 {
    ENABLED.load(Ordering::Relaxed)
}

/// Restore the enable bitmap from a persisted word, forcing every
/// [`FEATURE_ALWAYS_ON`] feature on regardless of the stored bit so a stale or
/// corrupt blob can never strand the structural core off.
pub fn set_enabled_map(map: u32) {
    let mut map = map;
    for f in FEATURES {
        if f.flags() & FEATURE_ALWAYS_ON != 0 {
            map |= enable_bit(f.id());
        }
    }
    ENABLED.store(map, Ordering::Relaxed);
}

/// Switch feature `raw_id` (a [`FeatureId`] discriminant on the wire) on or off.
/// Clears the feature's transient state ([`Feature::on_disable`]) on the off edge.
/// `Err` for an unknown id or an attempt to disable a [`FEATURE_ALWAYS_ON`] feature.
pub fn set_enabled(raw_id: u8, on: bool) -> Result<(), ()> {
    let f = FEATURES.iter().find(|f| f.id() as u8 == raw_id).ok_or(())?;
    if !on && f.flags() & FEATURE_ALWAYS_ON != 0 {
        return Err(());
    }
    let bit = enable_bit(f.id());
    if on {
        ENABLED.fetch_or(bit, Ordering::Relaxed);
    } else {
        ENABLED.fetch_and(!bit, Ordering::Relaxed);
        f.on_disable();
    }
    Ok(())
}

/// Serve the registry-owned `FEATURES` kcp group (`0xDx`): enumerate every feature's
/// `{id, enabled, name}` and toggle one. Auto-enumerates [`FEATURES`], so a newly
/// registered feature appears with no extra kcp wiring. `out` is the 29-byte reply
/// payload; `req` the request payload (`req[0]` is the first byte after `CMD`/`SEQ`).
///
/// * [`CMD_GET_FEATURES`](crate::kcp::CMD_GET_FEATURES) — request `[start]`; reply
///   `[count, page_len, {id, enabled, name_len, name…}…]` packing as many records
///   from `start` as fit one frame (the host pages until it has `count`).
/// * [`CMD_SET_FEATURE_ENABLED`](crate::kcp::CMD_SET_FEATURE_ENABLED) — request
///   `[id, 0|1]`; [`Status::BadArg`] for an unknown id, an always-on disable, or a
///   non-boolean value.
pub fn features_dispatch(cmd: u8, req: &[u8], out: &mut [u8]) -> Status {
    match cmd {
        crate::kcp::CMD_GET_FEATURES => {
            pack_features(req[0], out);
            Status::Ok
        }
        crate::kcp::CMD_SET_FEATURE_ENABLED => match req[1] {
            0 | 1 => match set_enabled(req[0], req[1] != 0) {
                Ok(()) => Status::Ok,
                Err(()) => Status::BadArg,
            },
            _ => Status::BadArg,
        },
        // Known group, unrecognised operation.
        _ => Status::BadCmd,
    }
}

/// Pack the feature enumeration from index `start` into the reply payload:
/// `out[0]` = total feature count, `out[1]` = records in this page, then each record
/// `[id, enabled, name_len, name_bytes…]` while the next one fits. The stable `id`
/// keys host state, never the array position, so the host pages by record count.
fn pack_features(start: u8, out: &mut [u8]) {
    out[0] = FEATURES.len() as u8;
    let mut pos = 2;
    let mut page = 0u8;
    for f in FEATURES.iter().skip(start as usize) {
        let name = f.name().as_bytes();
        if pos + 3 + name.len() > out.len() {
            break;
        }
        out[pos] = f.id() as u8;
        out[pos + 1] = is_enabled(f.id()) as u8;
        out[pos + 2] = name.len() as u8;
        out[pos + 3..pos + 3 + name.len()].copy_from_slice(name);
        pos += 3 + name.len();
        page += 1;
    }
    out[1] = page;
}

/// Run the matrix fold: each enabled, active feature transforms `m` in array order.
pub fn run_on_matrix(c: &Ctx, m: &mut [u16; NUM_ROWS]) {
    for f in FEATURES {
        if is_enabled(f.id()) && f.active() {
            f.on_matrix(c, m);
        }
    }
}

/// Run the report fold: each enabled, active feature rewrites `mods`/`keys` in array order.
pub fn run_on_report(c: &Ctx, mods: &mut u8, keys: &mut KeySet) {
    for f in FEATURES {
        if is_enabled(f.id()) && f.active() {
            f.on_report(c, mods, keys);
        }
    }
}

/// Run the overlay fold: each enabled, active feature merges its output into `r`.
pub fn run_on_overlay(c: &Ctx, r: &mut Report) {
    for f in FEATURES {
        if is_enabled(f.id()) && f.active() {
            f.on_overlay(c, r);
        }
    }
}

/// Run the RGB-frame claim (first-claims): the first active feature to paint and
/// return `true` owns the frame, vetoing the base effect. Returns whether the frame
/// was claimed. The base per-key effects are a separate registry
/// ([`crate::rgb::RGB_EFFECTS`]); this lets a feature paint a status overlay over
/// the whole panel instead.
pub fn run_on_rgb_frame(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) -> bool {
    for f in FEATURES {
        if is_enabled(f.id()) && f.active() && f.on_rgb_frame(c, leds) {
            return true;
        }
    }
    false
}

/// Dispatch a kcp request: the first feature to claim `cmd` wins; an unclaimed
/// command is [`Status::Unsupported`] (the same answer [`crate::kcp::handle`]
/// gives an unknown group). A feature that owns a whole command group returns
/// [`Status::BadCmd`] for an unrecognised operation within it, preserving the
/// per-group reply byte-for-byte.
pub fn run_on_kcp(cmd: u8, req: &[u8], out: &mut [u8]) -> Status {
    for f in FEATURES {
        if let Some(s) = f.on_kcp(cmd, req, out) {
            return s;
        }
    }
    Status::Unsupported
}

/// Run the per-scan tick (sequence) for every enabled, active feature.
pub fn run_on_tick(now: Instant) {
    for f in FEATURES {
        if is_enabled(f.id()) && f.active() {
            f.on_tick(now);
        }
    }
}

/// Snapshot every feature's state into the config blob `out` (sequence, ungated:
/// an idle feature still writes its deterministic empty region).
pub fn run_on_save(out: &mut [u8]) {
    for f in FEATURES {
        f.on_save(out);
    }
}

/// Restore every feature's state from the config blob `buf` (sequence, ungated: a
/// feature must rebuild its table even though it is idle until it does).
pub fn run_on_load(buf: &[u8]) {
    for f in FEATURES {
        f.on_load(buf);
    }
}
