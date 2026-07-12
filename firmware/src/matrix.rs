// SPDX-License-Identifier: GPL-2.0-or-later
//! 6x15 `ROW2COL` key-matrix scanner for the Akko 5075B.
//!
//! # ROW2COL polarity
//!
//! This is a faithful port of QMK's `quantum/matrix.c` `ROW2COL` path
//! (`select_col`, `unselect_col`, `matrix_read_rows_on_col`) with
//! `MATRIX_INPUT_PRESSED_STATE == 0`. The convention is:
//!
//! * **Rows are inputs with pull-ups.** A released key floats the row high.
//! * **A column is "selected" by driving it output-low.** With a diode from
//!   row to column (ROW2COL), a pressed key on the selected column pulls its
//!   row input down to ground, so a **row that reads low (`0`) is pressed**
//!   (QMK's `readMatrixPin(...) == MATRIX_INPUT_PRESSED_STATE`).
//! * **Unselected columns are released to pulled-up inputs** (QMK's default,
//!   i.e. without `MATRIX_UNSELECT_DRIVE_HIGH`): high-impedance, weakly high.
//!
//! So one scan is, per column: select (drive low) -> settle -> read all six
//! rows (low => pressed) -> unselect (input pull-up). [`scan`] returns one
//! 15-bit bitmap per row, where bit `c` is set when the key at `(row, c)` is
//! pressed — matching QMK's `current_matrix[row] |= (1 << col)`.

use core::sync::atomic::{AtomicU8, Ordering};

use crate::clock;
use crate::gpio::{self, Pin, Port};
use pac::Peripherals;

/// Number of matrix rows.
pub const NUM_ROWS: usize = 6;
/// Number of matrix columns.
pub const NUM_COLS: usize = 15;

/// Row pins (inputs, pull-up). Order defines the row index.
pub const ROWS: [(Port, u8); NUM_ROWS] = [
    (Port::A, 0),
    (Port::A, 1),
    (Port::A, 2),
    (Port::A, 3),
    (Port::A, 4),
    (Port::C, 13),
];

/// Column pins (driven low when selected, pulled-up input otherwise). Order
/// defines the column index (the bit position within each row bitmap).
pub const COLS: [(Port, u8); NUM_COLS] = [
    (Port::C, 0),
    (Port::C, 1),
    (Port::C, 2),
    (Port::C, 3),
    (Port::A, 6),
    (Port::B, 10),
    (Port::B, 11),
    (Port::B, 12),
    (Port::B, 13),
    (Port::B, 14),
    (Port::A, 10),
    (Port::C, 6),
    (Port::C, 7),
    (Port::C, 8),
    (Port::C, 9),
];

/// Core cycles for the post-*select* settle (~1 us at 96 MHz): the active drive
/// of a freshly selected column, QMK's `matrix_output_select_delay`.
const SETTLE_CYCLES: u32 = clock::HCLK_HZ / 1_000_000;

/// Core cycles for the post-*unselect* settle (~30 us at 96 MHz). A row that a
/// pressed key pulled low must recover to V_IH through its weak internal pull-up
/// (against the row/diode/trace capacitance, tau ~2 us) before the next column
/// is read; too short a wait smears the low forward across later columns as
/// phantom presses. Matches QMK's `MATRIX_IO_DELAY` default (`wait_us(30)`).
const UNSELECT_CYCLES: u32 = clock::HCLK_HZ / 1_000_000 * 30;

/// Matrix debounce algorithm: how a raw level change becomes a debounced edge.
///
/// The discriminants are the kcp wire values (`CONFIG_SET_DEBOUNCE` `algorithm`
/// byte); decode an untrusted byte with [`DebounceAlgorithm::from_u8`]. Selected
/// live over kcp ([`crate::kcp`] CONFIG group) and persisted in the config blob
/// ([`crate::config`]).
#[derive(Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DebounceAlgorithm {
    /// Symmetric deferred filter: a change — press *or* release — is accepted only
    /// after [`interval`] consecutive scans agree on the new level. Noise-resistant
    /// on both edges; the firmware default and the pre-configurable behaviour.
    SymmetricDefer = 0,
    /// Asymmetric eager-on-press: a press is accepted on the first scan that sees
    /// it (no added latency), while a release is still deferred until [`interval`]
    /// consecutive released scans. Snappy for gaming, still chatter-free on release.
    AsymmetricEager = 1,
}

impl DebounceAlgorithm {
    /// Power-on default: the noise-resistant symmetric filter (unchanged behaviour).
    pub const DEFAULT: Self = Self::SymmetricDefer;

    /// Decode the kcp/blob `algorithm` byte; `None` for an unassigned value (the
    /// CONFIG group maps that to [`Status::BadArg`](crate::kcp::Status::BadArg)).
    pub fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::SymmetricDefer),
            1 => Some(Self::AsymmetricEager),
            _ => None,
        }
    }
}

/// Default deferred-edge interval: consecutive agreeing samples required before a
/// change is accepted (~5 ms at the 1 kHz scan). The pre-configurable firmware's
/// fixed sample count, kept as the power-on default so behaviour is unchanged until
/// the host sets otherwise.
pub const DEFAULT_DEBOUNCE_INTERVAL: u8 = 5;

// Live debounce config. Each field is an independent atomic with no cross-field
// invariant, so — as in [`crate::rgb`] — single-core `Relaxed` access is all the
// synchronisation needed between the kcp loop (writer) and the scan loop (reader).
/// Live debounce algorithm (one [`DebounceAlgorithm`] code), read each scan by
/// [`Debouncer::update`].
static DEBOUNCE_ALGORITHM: AtomicU8 = AtomicU8::new(DebounceAlgorithm::DEFAULT as u8);
/// Live deferred-edge interval in consecutive samples (`>= 1`).
static DEBOUNCE_INTERVAL: AtomicU8 = AtomicU8::new(DEFAULT_DEBOUNCE_INTERVAL);

/// Current debounce algorithm (falls back to the default for an unknown stored
/// code, which a validated config blob never holds).
pub fn algorithm() -> DebounceAlgorithm {
    DebounceAlgorithm::from_u8(DEBOUNCE_ALGORITHM.load(Ordering::Relaxed))
        .unwrap_or(DebounceAlgorithm::DEFAULT)
}

/// Current deferred-edge interval in consecutive samples.
pub fn interval() -> u8 {
    DEBOUNCE_INTERVAL.load(Ordering::Relaxed)
}

/// Set the live debounce algorithm and interval, applied on the next scan.
///
/// Returns `false` (changing nothing) for an unknown algorithm code or a zero
/// interval, so the kcp handler can report [`Status::BadArg`](crate::kcp::Status::BadArg).
/// A zero interval is rejected because a deferred edge needs at least one confirming
/// sample to mean anything.
#[must_use]
pub fn set_debounce(algorithm: u8, interval: u8) -> bool {
    let Some(algorithm) = DebounceAlgorithm::from_u8(algorithm) else {
        return false;
    };
    if interval == 0 {
        return false;
    }
    DEBOUNCE_ALGORITHM.store(algorithm as u8, Ordering::Relaxed);
    DEBOUNCE_INTERVAL.store(interval, Ordering::Relaxed);
    true
}

/// Restore the power-on debounce defaults (symmetric filter, default interval).
/// Used by the kcp CONFIG reset-to-defaults path
/// ([`crate::config::reset_to_defaults`]).
pub fn reset_debounce_defaults() {
    DEBOUNCE_ALGORITHM.store(DebounceAlgorithm::DEFAULT as u8, Ordering::Relaxed);
    DEBOUNCE_INTERVAL.store(DEFAULT_DEBOUNCE_INTERVAL, Ordering::Relaxed);
}

/// Configure the matrix pins and enable the GPIO clocks.
///
/// Rows become pulled-up inputs and columns start unselected (also pulled-up
/// inputs), exactly as QMK's `matrix_init_pins` leaves them for ROW2COL.
pub fn init(p: &Peripherals) {
    gpio::enable_clocks(p);
    for &(port, num) in ROWS.iter() {
        gpio::set_input_pull_up(Pin::new(port, num));
    }
    for &(port, num) in COLS.iter() {
        gpio::set_input_pull_up(Pin::new(port, num));
    }
}

/// Scan the matrix once and return the raw (un-debounced) state: one 15-bit
/// bitmap per row, bit `c` set when key `(row, c)` is pressed.
pub fn scan() -> [u16; NUM_ROWS] {
    let mut matrix = [0u16; NUM_ROWS];

    for (c, &(cport, cnum)) in COLS.iter().enumerate() {
        let col = Pin::new(cport, cnum);

        // Select the column: drive it low (QMK `gpio_atomic_set_pin_output_low`).
        gpio::set_output_push_pull(col);
        gpio::set_low(col);
        cortex_m::asm::delay(SETTLE_CYCLES);

        // Read every row: a low level means the key is pressed.
        for (r, &(rport, rnum)) in ROWS.iter().enumerate() {
            if gpio::is_low(Pin::new(rport, rnum)) {
                matrix[r] |= 1u16 << c;
            }
        }

        // Unselect the column: release it to a pulled-up input
        // (QMK `gpio_atomic_set_pin_input_high`), then let the rows recover
        // through their weak pull-ups before the next column is read.
        gpio::set_input_pull_up(col);
        cortex_m::asm::delay(UNSELECT_CYCLES);
    }

    matrix
}

/// Per-key matrix debouncer driving the live [`algorithm`] and [`interval`].
///
/// Holds the debounced state plus a per-key counter of consecutive scans
/// disagreeing with it; [`update`](Self::update) folds one raw scan in under the
/// currently selected [`DebounceAlgorithm`]. The default
/// ([`DebounceAlgorithm::SymmetricDefer`] at [`DEFAULT_DEBOUNCE_INTERVAL`]) is the
/// pre-configurable symmetric filter — clean edges on both press and release.
pub struct Debouncer {
    /// Debounced state, one 15-bit bitmap per row.
    state: [u16; NUM_ROWS],
    /// Per-key count of consecutive samples disagreeing with `state`.
    counters: [[u8; NUM_COLS]; NUM_ROWS],
}

impl Debouncer {
    /// Create a debouncer with all keys released.
    pub const fn new() -> Self {
        Self {
            state: [0; NUM_ROWS],
            counters: [[0; NUM_COLS]; NUM_ROWS],
        }
    }

    /// Fold one raw scan into the debounced state and return the latter.
    ///
    /// Reads the live [`algorithm`] and [`interval`] each call, so a host
    /// `CONFIG_SET_DEBOUNCE` takes effect on the very next scan without rebuilding
    /// the debouncer. Under [`DebounceAlgorithm::SymmetricDefer`] both edges are
    /// deferred until `interval` consecutive disagreeing scans; under
    /// [`DebounceAlgorithm::AsymmetricEager`] a press is taken immediately and only
    /// the release is deferred. The per-key counter tracks consecutive scans
    /// disagreeing with the stable state and resets whenever the raw level matches it
    /// again, so a bounce shorter than the interval never registers.
    pub fn update(&mut self, raw: [u16; NUM_ROWS]) -> [u16; NUM_ROWS] {
        let eager = algorithm() == DebounceAlgorithm::AsymmetricEager;
        let interval = interval();
        for r in 0..NUM_ROWS {
            for c in 0..NUM_COLS {
                let bit = 1u16 << c;
                let raw_pressed = raw[r] & bit != 0;
                let stable_pressed = self.state[r] & bit != 0;

                if raw_pressed == stable_pressed {
                    // Agrees with the stable state: reset the bounce counter.
                    self.counters[r][c] = 0;
                    continue;
                }

                // A press under the eager algorithm is accepted on this first scan;
                // every other edge (any release, or any edge under the symmetric
                // algorithm) waits for `interval` consecutive disagreeing scans.
                if (eager && raw_pressed) || self.counters[r][c] + 1 >= interval {
                    self.state[r] ^= bit;
                    self.counters[r][c] = 0;
                    // Reactive-RGB hook: feed the debounced *press* edge (not the
                    // release) to the per-key hit table so [`crate::rgb`] can light
                    // the pressed key. The one cross-module call here; the scanner
                    // is otherwise independent of RGB.
                    if self.state[r] & bit != 0 {
                        crate::rgb::note_key_press(r, c);
                    }
                } else {
                    self.counters[r][c] += 1;
                }
            }
        }
        self.state
    }
}
