// SPDX-License-Identifier: GPL-2.0-or-later
//! One-Shot-Mod (`OSM`) — a modifier that applies to just the next key.
//!
//! A press of an `OSM` key ([`KeyAction::OneShotMod`](crate::keycode::KeyAction::OneShotMod),
//! fired on its press edge by [`crate::keymap::compute_report`]) arms a modifier. The
//! modifier then latches onto the **next freshly-pressed key** — the keymap's edge loop
//! calls [`consume_press`](OneShotMod::consume_press) on each new basic-key press — and the
//! report fold ([`Feature::on_report`]) folds it in while that one key is held, releasing it
//! the moment that key lifts. It is the modifier analogue of one-shot-layer (`OSL`). Several
//! `OSM` presses stack (each ORs its bit) until a key consumes them, so `OSM(Ctrl)` then
//! `OSM(Shift)` shifts and controls the next key.
//!
//! # Why it waits for a *fresh* key
//!
//! Arming alone asserts nothing: a key already held when the `OSM` is pressed is *not*
//! latched onto (it was not a fresh press after arming), and an arm with no following key
//! never leaves a modifier stuck on the host. Only a key pressed *after* the arm consumes the
//! modifier, and only *that* key — tracked by its keycode — keeps it asserted, so the
//! modifier and its one key leave the report together. This is the standard one-shot
//! "applies to the next key, then auto-releases" behaviour.
//!
//! # State and idle cost
//!
//! Three atomics (the armed mask, the live mask, the consuming keycode) — no table, no lock,
//! no kcp, no persistence. `active()` is two relaxed loads and is false whenever nothing is
//! armed or live, so the hook is wholly skipped at rest.

use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};

use crate::behavior::KeySet;
use crate::features::{Ctx, Feature, FeatureId};
use crate::keycode::Keycode;

/// One-shot-modifier feature: the armed mask, the live (in-use) mask and the keycode of
/// the key currently consuming the modifier.
pub struct OneShotMod {
    /// Modifier bits armed by `OSM` presses, awaiting the next fresh key press.
    pending: AtomicU8,
    /// Modifier bits currently folded into the report while the consuming key is held;
    /// cleared the moment that key lifts.
    live: AtomicU8,
    /// Raw encoding of the key that latched the modifier, so the fold can tell when that
    /// specific key (not merely *some* key) has lifted. Meaningful only while `live != 0`.
    consumer: AtomicU16,
}

/// The singleton in the [`FEATURES`](crate::features::FEATURES) registry.
pub static ONE_SHOT_MOD: OneShotMod = OneShotMod {
    pending: AtomicU8::new(0),
    live: AtomicU8::new(0),
    consumer: AtomicU16::new(0),
};

impl OneShotMod {
    /// Arm the HID modifier `bit` (`0..=7`) for the next key (the `OSM` press edge). Bits
    /// from several presses accumulate until a key consumes them.
    pub fn arm(&self, bit: u8) {
        // Inert while the feature is disabled, so a press queues no armed modifier that would
        // latch onto the next key on re-enable (the scan hook is already enable-gated).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        self.pending.fetch_or(1 << bit, Ordering::Relaxed);
    }

    /// Latch the armed modifier onto `kc`, a freshly-pressed key. Called from the keymap's
    /// edge loop for each new basic-key press, so the modifier binds to the *next* key after
    /// the arm — never one held through the arm. A no-op unless something is armed; once a key
    /// consumes the arm, later presses do not re-latch until a new `OSM` arms again.
    pub fn consume_press(&self, kc: Keycode) {
        // Inert while disabled (see `arm`): with nothing armed it is already a no-op, but
        // gating here keeps the keycode path single-source with the enable bit.
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        let pending = self.pending.load(Ordering::Relaxed);
        if pending != 0 {
            self.live.store(pending, Ordering::Relaxed);
            self.pending.store(0, Ordering::Relaxed);
            self.consumer.store(kc.raw(), Ordering::Relaxed);
        }
    }
}

impl Feature for OneShotMod {
    fn id(&self) -> FeatureId {
        FeatureId::OneShotMod
    }

    fn name(&self) -> &'static str {
        "One-Shot Mod"
    }

    /// Two relaxed loads: skipped unless a modifier is armed or currently applied.
    fn active(&self) -> bool {
        (self.pending.load(Ordering::Relaxed) | self.live.load(Ordering::Relaxed)) != 0
    }

    /// Drop any armed or live modifier when the feature is switched off, so no
    /// one-shot lingers on the report and a re-enable starts unarmed.
    fn on_disable(&self) {
        self.pending.store(0, Ordering::Relaxed);
        self.live.store(0, Ordering::Relaxed);
        self.consumer.store(0, Ordering::Relaxed);
    }

    fn on_report(&self, _c: &Ctx, mods: &mut u8, keys: &mut KeySet) {
        let live = self.live.load(Ordering::Relaxed);
        if live == 0 {
            return;
        }
        // Fold the modifier in only while the consuming key is still held; the moment it
        // lifts, retire the one-shot so neither the key nor its modifier lingers.
        let consumer = self.consumer.load(Ordering::Relaxed);
        if keys.as_slice().iter().any(|kc| kc.raw() == consumer) {
            *mods |= live;
        } else {
            self.live.store(0, Ordering::Relaxed);
            self.consumer.store(0, Ordering::Relaxed);
        }
    }
}
