// SPDX-License-Identifier: GPL-2.0-or-later
//! Repeat Key — re-emit the last key (`Repeat`) or its alternate (`AltRepeat`).
//!
//! Every fresh basic-key press is remembered, together with the modifiers held with it,
//! by [`record_press`](RepeatKey::record_press) (called from
//! [`crate::keymap::compute_report`]'s resolve walk). A press of `Repeat`
//! ([`KeyAction::Repeat`](crate::keycode::KeyAction::Repeat)) re-emits that key+mods; a
//! press of `AltRepeat` ([`KeyAction::AltRepeat`](crate::keycode::KeyAction::AltRepeat))
//! re-emits its alternate from a small mapping table (e.g. Left for Right). The re-emit is
//! injected into the report fold ([`Feature::on_report`]) as a one-scan tap — pushed into
//! the working key set so [`compute_report`](crate::keymap::compute_report) finalises it
//! into the boot and NKRO reports like any resolved key.
//!
//! # Why capture runs in the overlay, not the fold
//!
//! The modifiers held with a key are only final *after* the modifier-producing report hooks
//! (Caps Word, One-Shot-Mod) have run. Capturing them in [`Feature::on_report`] — which runs
//! before those hooks in registry order — would miss a Caps-Word or `OSM(Shift)`, so a later
//! `Repeat` would repeat the key unshifted. The capture therefore runs in
//! [`Feature::on_overlay`], which sees the finished report's modifier byte (the truly final
//! modifiers); the inject stays in the fold so the re-emitted key is itself shifted by an
//! active Caps Word.
//!
//! # State and idle cost
//!
//! All state is plain atomics (the remembered key/mods, the pending tap), so there is no
//! table and no lock. `active()` is one relaxed load and is false except on the single
//! scan after a key press (to snapshot its final modifiers) or a `Repeat`/`AltRepeat` press
//! (to inject); the repeat keycodes carry no settings, so there is no kcp or persistence.

use core::sync::atomic::{AtomicU16, AtomicU8, Ordering};

use crate::behavior::KeySet;
use crate::features::{Ctx, Feature, FeatureId};
use crate::keycode::{KeyAction, Keycode};
use crate::keymap::Report;

/// `pending` bit: snapshot the final modifier byte into `last_mods` on the next overlay —
/// set when a fresh key is recorded. Captured in the overlay (not the fold) so it reflects
/// the modifiers Caps Word / One-Shot-Mod fold in, i.e. exactly those held with the key.
const CAPTURE_MODS: u8 = 0b01;
/// `pending` bit: inject `inject_key`+`inject_mods` into the next report (a one-scan tap).
const INJECT: u8 = 0b10;

/// Repeat-key feature: the remembered key+mods and a pending tap to inject.
pub struct RepeatKey {
    /// Raw encoding of the last freshly-pressed basic key.
    last_key: AtomicU16,
    /// Modifier byte held when [`last_key`](Self::last_key) was pressed.
    last_mods: AtomicU8,
    /// Raw encoding of the key to inject on the pending tap.
    inject_key: AtomicU16,
    /// Modifier byte to assert on the pending tap.
    inject_mods: AtomicU8,
    /// Pending work ([`CAPTURE_MODS`] | [`INJECT`]); zero means idle. The `active()` gate.
    pending: AtomicU8,
}

/// The singleton in the [`FEATURES`](crate::features::FEATURES) registry.
pub static REPEAT: RepeatKey = RepeatKey {
    last_key: AtomicU16::new(0),
    last_mods: AtomicU8::new(0),
    inject_key: AtomicU16::new(0),
    inject_mods: AtomicU8::new(0),
    pending: AtomicU8::new(0),
};

impl RepeatKey {
    /// Remember `kc` as the key a later `Repeat` repeats, and arm the modifier snapshot
    /// (taken in the overlay, once this scan's final modifiers are known). Called for every
    /// fresh basic-key press (never the repeat keys themselves, which resolve to no basic
    /// usage), so repeating always re-emits the previous real key.
    pub fn record_press(&self, kc: Keycode) {
        // Inert while the feature is disabled, so the remembered key never advances and a
        // queued capture / inject can never fire on re-enable (the scan hooks are already
        // enable-gated; this gates the keycode path too).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        self.last_key.store(kc.raw(), Ordering::Relaxed);
        self.pending.fetch_or(CAPTURE_MODS, Ordering::Relaxed);
    }

    /// Re-emit the remembered key with its remembered modifiers (the `Repeat` press edge).
    pub fn repeat(&self) {
        // Inert while disabled (see `record_press`).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        let last = Keycode::from_raw(self.last_key.load(Ordering::Relaxed));
        if matches!(last.classify(), KeyAction::Key(_)) {
            self.arm_inject(last);
        }
    }

    /// Re-emit the remembered key's alternate, if one is mapped (the `AltRepeat` press edge).
    pub fn alt_repeat(&self) {
        // Inert while disabled (see `record_press`).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        let last = Keycode::from_raw(self.last_key.load(Ordering::Relaxed));
        if let KeyAction::Key(usage) = last.classify() {
            if let Some(alt) = alt_usage(usage) {
                self.arm_inject(Keycode::from_usage(alt));
            }
        }
    }

    /// Queue `kc` (with the remembered modifiers) for the next report's one-scan tap.
    fn arm_inject(&self, kc: Keycode) {
        self.inject_key.store(kc.raw(), Ordering::Relaxed);
        self.inject_mods
            .store(self.last_mods.load(Ordering::Relaxed), Ordering::Relaxed);
        self.pending.fetch_or(INJECT, Ordering::Relaxed);
    }
}

/// The alternate-repeat mapping: each key's natural opposite, for `AltRepeat`. Kept small
/// and reversible (the cursor-movement and erase pairs), so alt-repeat undoes or mirrors
/// the last motion; `None` for a key with no alternate, where `AltRepeat` does nothing.
fn alt_usage(usage: u8) -> Option<u8> {
    Some(match usage {
        0x4F => 0x50, // Right -> Left
        0x50 => 0x4F, // Left -> Right
        0x52 => 0x51, // Up -> Down
        0x51 => 0x52, // Down -> Up
        0x4A => 0x4D, // Home -> End
        0x4D => 0x4A, // End -> Home
        0x4B => 0x4E, // Page Up -> Page Down
        0x4E => 0x4B, // Page Down -> Page Up
        0x2A => 0x4C, // Backspace -> Delete
        0x4C => 0x2A, // Delete -> Backspace
        _ => return None,
    })
}

impl Feature for RepeatKey {
    fn id(&self) -> FeatureId {
        FeatureId::RepeatKey
    }

    fn name(&self) -> &'static str {
        "Repeat Key"
    }

    /// One relaxed load: work is pending only on the scan after a key press or a repeat
    /// press; otherwise the feature is wholly skipped.
    fn active(&self) -> bool {
        self.pending.load(Ordering::Relaxed) != 0
    }

    /// Drop any pending inject / modifier-capture and the remembered key when the
    /// feature is switched off, so nothing is injected after a disable and a re-enable
    /// repeats only keys pressed since.
    fn on_disable(&self) {
        self.pending.store(0, Ordering::Relaxed);
        self.last_key.store(0, Ordering::Relaxed);
        self.last_mods.store(0, Ordering::Relaxed);
    }

    /// Inject the pending repeat tap into the fold (so the re-emitted key is itself shifted
    /// by an active Caps Word). The modifier capture is deferred to [`Self::on_overlay`],
    /// where this scan's final modifiers are known, so only the `INJECT` bit is cleared here.
    fn on_report(&self, _c: &Ctx, mods: &mut u8, keys: &mut KeySet) {
        if self.pending.load(Ordering::Relaxed) & INJECT != 0 {
            keys.push(Keycode::from_raw(self.inject_key.load(Ordering::Relaxed)));
            *mods |= self.inject_mods.load(Ordering::Relaxed);
            self.pending.fetch_and(!INJECT, Ordering::Relaxed);
        }
    }

    /// Snapshot the recorded key's modifiers from the finished report — the truly final
    /// modifier byte, after Caps Word / One-Shot-Mod (and the timed overlay) have folded
    /// theirs in — so a later `Repeat` repeats the keystroke exactly as it was sent.
    fn on_overlay(&self, _c: &Ctx, r: &mut Report) {
        if self.pending.load(Ordering::Relaxed) & CAPTURE_MODS != 0 {
            self.last_mods.store(r.boot.modifier, Ordering::Relaxed);
            self.pending.fetch_and(!CAPTURE_MODS, Ordering::Relaxed);
        }
    }
}
