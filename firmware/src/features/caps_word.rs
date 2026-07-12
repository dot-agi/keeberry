// SPDX-License-Identifier: GPL-2.0-or-later
//! Caps Word — Shift held for one word, ended by the first non-word key.
//!
//! A press of `CapsWord` ([`KeyAction::CapsWord`](crate::keycode::KeyAction::CapsWord),
//! fired on its press edge by [`crate::keymap::compute_report`]) engages the feature; the
//! report fold ([`Feature::on_report`]) then asserts Left Shift on every **letter** and
//! ends the word the moment a non-word key is pressed. Digits, `-` and Backspace continue
//! the word but are left unshifted, exactly as QMK's Caps Word does (so `cap-2` keeps the
//! caps run without uppercasing the digit). It is the canonical small plugin: its registry
//! wiring is one `feature!` block declaring a single `AtomicBool`, an
//! `active()` gate that costs one relaxed load when off, and one report hook — no table,
//! no kcp, no persistence (caps-word state is transient and intentionally never saved).
//!
//! # Soundness
//!
//! The only state is an [`AtomicBool`]; the hook is synchronous and holds no borrow
//! across an `.await`, so it is safe on the cooperative executor like every other
//! feature.

use core::sync::atomic::{AtomicBool, Ordering};

use crate::behavior::KeySet;
use crate::features::{is_hid_letter, Ctx, Feature, FeatureId, FEATURE_DEFAULT_ON};
use crate::keycode::KeyAction;

feature! {
    /// Caps-word feature: holds Left Shift across one word once engaged.
    CapsWord as CAPS_WORD,
    id = FeatureId::CapsWord,
    name = "Caps Word",
    flags = FEATURE_DEFAULT_ON,
    state = {
        /// Whether caps-word is currently engaged. The `active()` fast-path flag.
        on: AtomicBool = AtomicBool::new(false),
    },
    hooks = {
        /// One relaxed load: the feature is wholly skipped until a `CapsWord` press
        /// engages it.
        fn active(&self) -> bool {
            self.on.load(Ordering::Relaxed)
        }

        /// Drop any engaged caps run when the feature is switched off, so it leaves no
        /// latent Shift and a re-enable starts disengaged.
        fn on_disable(&self) {
            self.end();
        }

        /// Assert Left Shift on each letter in the resolved set; digits, `-` and Backspace
        /// pass through unshifted but keep the word alive, and the first key that is none of
        /// those ends the word so the next key types unshifted.
        fn on_report(&self, _c: &Ctx, mods: &mut u8, keys: &mut KeySet) {
            let mut terminate = false;
            for kc in keys.as_slice() {
                if let KeyAction::Key(usage) = kc.classify() {
                    if is_hid_letter(usage) {
                        *mods |= 1 << 1;
                    } else if !continues_unshifted(usage) {
                        terminate = true;
                    }
                }
            }
            if terminate {
                self.end();
            }
        }
    },
}

impl CapsWord {
    /// Engage caps-word. Called on the `CapsWord` press edge; the word then runs
    /// until [`end`](Self::end) retires it on the first non-word key.
    pub fn engage(&self) {
        // Inert while the feature is disabled, so a press queues no state that would fire on
        // re-enable (the scan hooks are already enable-gated; this gates the keycode path too).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        self.on.store(true, Ordering::Relaxed);
    }

    /// Retire caps-word (a non-word key ended the word).
    fn end(&self) {
        self.on.store(false, Ordering::Relaxed);
    }
}

/// Whether `usage` continues the word *without* a Shift: the digits (`1`–`0`), `-`
/// and Backspace. Letters continue too (and are shifted by [`is_hid_letter`]); every
/// other basic key ends the word.
fn continues_unshifted(usage: u8) -> bool {
    matches!(usage, 0x1E..=0x27 | 0x2D | 0x2A)
}
