// SPDX-License-Identifier: GPL-2.0-or-later
//! Key Lock — latch the next key down until it is pressed again.
//!
//! A press of `KeyLock` ([`KeyAction::KeyLock`](crate::keycode::KeyAction::KeyLock), fired
//! on its press edge by [`crate::keymap::compute_report`]) arms the feature; the very next
//! key press is captured into a small locked set and held down — its matrix bit is folded
//! back in every scan ([`Feature::on_matrix`]) so the report builder keeps resolving and
//! emitting it — until that key is physically pressed again, which unlocks it. This is the
//! one-handed-Shift / hold-a-key trick QMK's Key Lock provides.
//!
//! # Why positions, and why the matrix fold
//!
//! The locked unit is the matrix **position**, not a resolved keycode: re-asserting its
//! bit lets [`compute_report`](crate::keymap::compute_report) resolve it through the live
//! layers exactly as a physical hold would, so a locked modifier folds into the modifier
//! byte and a locked basic key into the key set with no special-casing here. The arming
//! key's own position is remembered ([`arm`](KeyLock::arm)) and skipped during capture, so
//! re-tapping `KeyLock` before choosing a key never latches the lock key onto itself.
//!
//! Edges are read from the *physical* `prev_matrix` the keyboard loop carries (never the
//! folded matrix), so a held lock re-asserted each scan can still be told apart from a
//! fresh physical press of the same key — that press is the unlock.
//!
//! # Soundness
//!
//! The locked set lives behind a blocking [`Mutex`] + [`RefCell`] like every other table
//! ([`crate::behavior`]); the hook is synchronous and holds the borrow only across the
//! in-place edits, never across an `.await`.

use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU16, Ordering};

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;

use crate::features::{Ctx, Feature, FeatureId, FEATURE_DEFAULT_ON};
use crate::matrix::NUM_ROWS;

/// Maximum number of simultaneously locked keys.
const MAX_LOCKED: usize = 8;

/// Pack a matrix position into one `u16` (`row` high byte, `col` low byte) so the locked
/// set and the armed-key marker are plain atomics / `Copy` slots.
const fn pack(row: u8, col: u8) -> u16 {
    ((row as u16) << 8) | col as u16
}

feature! {
    /// Key-lock feature: a set of latched matrix positions plus the armed flag.
    KeyLock as KEY_LOCK,
    id = FeatureId::KeyLock,
    name = "Key Lock",
    flags = FEATURE_DEFAULT_ON,
    state = {
        /// The latched positions (packed by [`pack`]); `None` is an empty slot.
        locked: Mutex<CriticalSectionRawMutex, RefCell<[Option<u16>; MAX_LOCKED]>> =
            Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new([None; MAX_LOCKED])),
        /// Whether the next fresh key press should be latched.
        armed: AtomicBool = AtomicBool::new(false),
        /// The arming `KeyLock` key's own position (packed), skipped during capture; only
        /// meaningful while [`armed`](Self::armed) is set.
        arm_pos: AtomicU16 = AtomicU16::new(0),
        /// Fast-path flag: armed, or at least one key latched. The `active()` gate.
        any: AtomicBool = AtomicBool::new(false),
    },
    hooks = {
        /// One relaxed load: skipped entirely until armed or holding a latched key.
        fn active(&self) -> bool {
            self.any.load(Ordering::Relaxed)
        }

        /// Release every latched position and disarm when the feature is switched off, so
        /// no key is left held and a re-enable starts with an empty lock set.
        fn on_disable(&self) {
            self.armed.store(false, Ordering::Relaxed);
            self.any.store(false, Ordering::Relaxed);
            self.locked.lock(|cell| *cell.borrow_mut() = [None; MAX_LOCKED]);
        }

        fn on_matrix(&self, c: &Ctx, m: &mut [u16; NUM_ROWS]) {
            let armed = self.armed.load(Ordering::Relaxed);
            let arm_pos = self.arm_pos.load(Ordering::Relaxed);
            self.locked.lock(|cell| {
                let mut table = cell.borrow_mut();
                let mut captured = false;
                for (r, &prev) in c.prev_matrix.iter().enumerate() {
                    let mut press = m[r] & !prev;
                    while press != 0 {
                        let col = press.trailing_zeros() as u8;
                        press &= press - 1;
                        let pos = pack(r as u8, col);
                        // A fresh press of an already-latched key unlocks it.
                        if let Some(slot) = table.iter_mut().find(|s| **s == Some(pos)) {
                            *slot = None;
                            continue;
                        }
                        // The first fresh key while armed (never the lock key itself) latches.
                        if armed && !captured && pos != arm_pos {
                            if let Some(slot) = table.iter_mut().find(|s| s.is_none()) {
                                *slot = Some(pos);
                                captured = true;
                            }
                        }
                    }
                }
                if captured {
                    self.armed.store(false, Ordering::Relaxed);
                }
                // Re-assert every latched position so the report keeps emitting it.
                for &pos in table.iter().flatten() {
                    m[(pos >> 8) as usize] |= 1 << (pos & 0xFF);
                }
                self.any.store(
                    self.armed.load(Ordering::Relaxed) || table.iter().any(Option::is_some),
                    Ordering::Relaxed,
                );
            });
        }
    },
}

impl KeyLock {
    /// Arm key-lock: the next fresh key press latches. Called on the `KeyLock` press edge
    /// with that key's `(row, col)`, which is then excluded from capture.
    pub fn arm(&self, row: u8, col: u8) {
        // Inert while the feature is disabled, so a press queues no armed state that would
        // latch a key on re-enable (the scan hook is already enable-gated).
        if !crate::features::is_enabled(self.id()) {
            return;
        }
        self.arm_pos.store(pack(row, col), Ordering::Relaxed);
        self.armed.store(true, Ordering::Relaxed);
        self.any.store(true, Ordering::Relaxed);
    }
}
