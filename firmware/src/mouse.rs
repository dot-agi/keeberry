// SPDX-License-Identifier: GPL-2.0-or-later
//! USB HID mouse keys: a small, stateful pointer accelerator.
//!
//! keeberry has no pointing device, but the keymap can bind the nine [`MouseKey`]
//! actions ([`crate::keycode`]): four-way pointer movement, the three buttons, and
//! wheel up/down. They carry no keyboard usage, so [`crate::keymap::compute_report`]
//! ignores them; instead [`crate::keymap::mouse_keys`] resolves the *currently held*
//! mouse keys each scan into the compact [bitmask](self#bitmask) this module
//! decodes, and the shared report-ID interface ([`crate::usb`]) sends mouse HID
//! reports (report ID 4) built from it — no new USB endpoint, the mouse report
//! rides EP3 alongside NKRO, consumer and system control.
//!
//! # Why an accelerator, and where it runs
//!
//! HID pointer movement is *relative*: each report moves the cursor by its signed
//! X/Y delta, so continuous motion means emitting a stream of small deltas while a
//! direction key is held. A fixed delta feels either too slow to cross the screen
//! or too coarse for fine aiming, so [`Accel`] ramps the per-report delta from
//! [`MOVE_BASE`] up to [`MOVE_MAX`] the longer a direction is held — the standard
//! mouse-keys behaviour. The wheel scrolls one detent per [`WHEEL_INTERVAL_MS`].
//!
//! [`Accel`] is owned as a local by [`crate::usb`]'s shared-interface loop (the
//! sender), not a global: that loop already paces the EP3 writes, so stepping the
//! accelerator there keeps all of the movement timing on the one task that emits
//! it — there is no cross-task edge hand-off to race. The held-key *bitmask* is
//! level state, published once per scan like the consumer usage, so sampling it
//! from the send loop is race-free.
//!
//! # Bitmask
//!
//! The held mouse keys are carried as a `u16` whose low nine bits are the `M_*`
//! flags below (one per [`MouseKey`]). [`crate::keymap::mouse_keys`] builds it from
//! the matrix; [`buttons`] and [`Accel::step`] decode it.
//!
//! # Buttons vs. movement
//!
//! Buttons are *absolute* HID state (a held bit), so a release must be reported;
//! movement and wheel are *relative* (a one-shot delta), so stopping is simply
//! ceasing to send. The send loop therefore dedups buttons against the last value
//! it sent and always emits a non-zero movement/wheel delta — [`Accel::step`]
//! returns only the deltas, leaving that policy to the caller.
//!
//! Scope: USB only. There is no vendor radio frame for a mouse, so on a wireless
//! transport the held mouse keys simply do not emit (the send loop skips the mouse
//! report and resets the accelerator on the switch).

use embassy_time::Duration;

// === Held-mouse-key bitmask (one bit per `MouseKey`) ========================

/// Move up is held.
pub const M_UP: u16 = 1 << 0;
/// Move down is held.
pub const M_DOWN: u16 = 1 << 1;
/// Move left is held.
pub const M_LEFT: u16 = 1 << 2;
/// Move right is held.
pub const M_RIGHT: u16 = 1 << 3;
/// Button 1 (left) is held.
pub const M_BTN1: u16 = 1 << 4;
/// Button 2 (right) is held.
pub const M_BTN2: u16 = 1 << 5;
/// Button 3 (middle) is held.
pub const M_BTN3: u16 = 1 << 6;
/// Wheel up is held.
pub const M_WHEEL_UP: u16 = 1 << 7;
/// Wheel down is held.
pub const M_WHEEL_DOWN: u16 = 1 << 8;

/// The HID mouse button byte from the held-key bitmask: bit 0 = button 1 (left),
/// bit 1 = button 2 (right), bit 2 = button 3 (middle), the standard boot-mouse
/// layout.
pub fn buttons(keys: u16) -> u8 {
    let mut b = 0u8;
    if keys & M_BTN1 != 0 {
        b |= 1 << 0;
    }
    if keys & M_BTN2 != 0 {
        b |= 1 << 1;
    }
    if keys & M_BTN3 != 0 {
        b |= 1 << 2;
    }
    b
}

// === Acceleration tuning ====================================================

/// Time between movement reports while a direction is held (~125 Hz). Small enough
/// to feel smooth, large enough that each report carries a meaningful delta.
const MOVE_INTERVAL_MS: u64 = 8;
/// Per-report delta on the first movement report (the slow, precise start).
const MOVE_BASE: i32 = 2;
/// Added to the per-report delta on each successive report (the ramp rate).
const MOVE_ACCEL: i32 = 1;
/// Ceiling for the per-report delta, well inside a signed byte so X and Y (and
/// their diagonal sum of magnitudes) never saturate the report field.
const MOVE_MAX: i32 = 24;
/// Ramp-index cap: the tick at which the delta first reaches [`MOVE_MAX`]. The ramp
/// index stops here (the delta is already saturated), so a long hold can never
/// overflow it.
const MOVE_TICKS_MAX: i32 = (MOVE_MAX - MOVE_BASE) / MOVE_ACCEL;
/// Time between wheel detents while a wheel key is held (~12 lines/second).
const WHEEL_INTERVAL_MS: u64 = 80;

/// Clamp a computed delta into the signed-byte range the HID mouse report carries.
fn clamp_i8(v: i32) -> i8 {
    v.clamp(-127, 127) as i8
}

/// Sign of an axis from its two opposing held flags: `+1` positive only, `-1`
/// negative only, `0` if neither or both (the SOCD-style mutual cancel that keeps
/// e.g. left+right from drifting).
fn axis(keys: u16, positive: u16, negative: u16) -> i32 {
    i32::from(keys & positive != 0) - i32::from(keys & negative != 0)
}

/// The mouse pointer/wheel accelerator: turns the held-key bitmask into the signed
/// movement and wheel deltas to send this tick, ramping movement speed the longer a
/// direction is held.
///
/// Stepped once per send-loop iteration with the real elapsed time, so the cadence
/// is correct even when a slow host stretches the loop. Holds only timing state;
/// the button policy lives in the caller (see the [module docs](self)).
pub struct Accel {
    /// Whether a movement direction was held last step (to detect the start, which
    /// emits immediately for a responsive first nudge).
    moving: bool,
    /// Movement reports emitted since the current motion began — the ramp index.
    move_ticks: i32,
    /// Time accumulated toward the next movement report.
    move_elapsed: Duration,
    /// Whether a wheel direction was held last step (start emits immediately).
    wheel_active: bool,
    /// Time accumulated toward the next wheel detent.
    wheel_elapsed: Duration,
}

impl Accel {
    /// The idle accelerator: nothing held, no motion in progress.
    pub const fn new() -> Self {
        Self {
            moving: false,
            move_ticks: 0,
            move_elapsed: Duration::from_ticks(0),
            wheel_active: false,
            wheel_elapsed: Duration::from_ticks(0),
        }
    }

    /// Reset to idle. Called when the USB interface is (re)configured or the
    /// transport switches, so motion never carries stale ramp state across the gap.
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    /// Advance by `dt` and return `(x, y, wheel)` to send this tick — each `0` when
    /// no report is due on that channel.
    ///
    /// Movement: the first held step emits at [`MOVE_BASE`]; thereafter a report is
    /// due every [`MOVE_INTERVAL_MS`], its delta ramping by [`MOVE_ACCEL`] up to
    /// [`MOVE_MAX`]. Releasing every direction resets the ramp. Wheel: one detent on
    /// the first held step, then one every [`WHEEL_INTERVAL_MS`]. X/Y/wheel are
    /// relative, so a `0` simply means "no motion this tick" — the cursor holds.
    pub fn step(&mut self, keys: u16, dt: Duration) -> (i8, i8, i8) {
        let sx = axis(keys, M_RIGHT, M_LEFT);
        let sy = axis(keys, M_DOWN, M_UP);
        let (mut x, mut y) = (0i32, 0i32);
        if sx == 0 && sy == 0 {
            // No direction held: motion stops and the ramp resets for next time.
            self.moving = false;
            self.move_ticks = 0;
            self.move_elapsed = Duration::from_ticks(0);
        } else if self.due(true, dt) {
            let speed = (MOVE_BASE + self.move_ticks * MOVE_ACCEL).min(MOVE_MAX);
            self.move_ticks = (self.move_ticks + 1).min(MOVE_TICKS_MAX);
            x = sx * speed;
            y = sy * speed;
        }

        let sw = axis(keys, M_WHEEL_UP, M_WHEEL_DOWN);
        let mut w = 0i32;
        if sw == 0 {
            self.wheel_active = false;
            self.wheel_elapsed = Duration::from_ticks(0);
        } else if self.due(false, dt) {
            w = sw;
        }

        (clamp_i8(x), clamp_i8(y), clamp_i8(w))
    }

    /// Whether the movement (`move_axis == true`) or wheel channel should emit this
    /// step: immediately when the channel just became active, else once its interval
    /// has accumulated. Advances that channel's elapsed/active state as a side effect.
    fn due(&mut self, move_axis: bool, dt: Duration) -> bool {
        let (active, elapsed, interval) = if move_axis {
            (
                &mut self.moving,
                &mut self.move_elapsed,
                Duration::from_millis(MOVE_INTERVAL_MS),
            )
        } else {
            (
                &mut self.wheel_active,
                &mut self.wheel_elapsed,
                Duration::from_millis(WHEEL_INTERVAL_MS),
            )
        };
        if !*active {
            *active = true;
            *elapsed = Duration::from_ticks(0);
            return true;
        }
        *elapsed += dt;
        if *elapsed >= interval {
            *elapsed = Duration::from_ticks(0);
            true
        } else {
            false
        }
    }
}

impl Default for Accel {
    fn default() -> Self {
        Self::new()
    }
}
