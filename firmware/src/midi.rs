// SPDX-License-Identifier: GPL-2.0-or-later
//! USB-MIDI mode: the key matrix as a chromatic MIDI controller.
//!
//! MIDI is a USB class with its own bulk endpoints, not a HID report ID, so it
//! cannot ride the shared EP3 interface the way the mouse / gamepad / digitizer
//! do — all three interrupt-IN endpoints are already spent by the normal
//! composite. MIDI therefore lives behind a *re-enumerated* USB mode
//! ([`crate::usb::UsbMode::Midi`]): the keyboard detaches, rebuilds its descriptor
//! as a [`MidiClass`] device (one bulk IN + one bulk OUT) alongside the kcp
//! control interface, and runs this loop in place of the keyboard/HID loops.
//! Entering and leaving the mode is the kcp
//! [`SYSTEM.SET_USB_MODE`](crate::kcp::CMD_SYSTEM_SET_USB_MODE) command.
//!
//! # Mapping (built-in chromatic layout)
//!
//! Each matrix key is one chromatic semitone: note `BASE_NOTE + row*NUM_COLS +
//! col`, so the physical grid reads left-to-right, top-to-bottom as ascending
//! pitch. Switches are digital, so every note-on uses a fixed [`VELOCITY`]. The
//! one knob position ([`KNOB`]) is not a note — it sends a control change on
//! [`KNOB_CC`] (full-scale while held, zero on release), the conventional
//! key-matrix stand-in for a continuous controller.

use crate::matrix::{self, NUM_COLS, NUM_ROWS};
use crate::usb::Driver;
use embassy_time::{Duration, Timer};
use embassy_usb::class::midi::MidiClass;

/// Fixed note-on velocity. The matrix switches are digital (on/off) with no
/// pressure, so every note strikes at one velocity; `100` is a firm mezzo-forte.
const VELOCITY: u8 = 100;

/// Lowest note in the chromatic grid (MIDI 36 = C2). The largest key index is
/// `(NUM_ROWS-1)*NUM_COLS + (NUM_COLS-1)`; with a 6×15 matrix that is 89, so the
/// top note is `36 + 89 = 125`, inside the 0..=127 MIDI range (asserted below).
const BASE_NOTE: u8 = 36;

const _: () = assert!(
    (BASE_NOTE as usize) + (NUM_ROWS - 1) * NUM_COLS + (NUM_COLS - 1) <= 127,
    "chromatic grid must stay within the 0..=127 MIDI note range"
);

/// MIDI channel 0 status nibbles (cable 0). USB-MIDI wraps each message in a
/// 4-byte event packet whose first byte is `cable<<4 | code-index`; the
/// code-index equals the status high nibble for these voice messages.
const NOTE_ON: u8 = 0x90;
const NOTE_OFF: u8 = 0x80;
const CONTROL_CHANGE: u8 = 0xB0;

/// The knob's matrix position (row 0, column 14). Mapped to a control change
/// rather than a note.
const KNOB: (usize, usize) = (0, 14);
/// Control-change number the knob drives (CC 1, the modulation wheel).
const KNOB_CC: u8 = 1;

/// The MIDI note for a matrix position, or `None` for the knob (which is a CC,
/// not a note). The grid is row-major chromatic from [`BASE_NOTE`].
fn note_for(row: usize, col: usize) -> Option<u8> {
    if (row, col) == KNOB {
        return None;
    }
    Some(BASE_NOTE + (row * NUM_COLS + col) as u8)
}

/// Build the 4-byte USB-MIDI event packet for a channel-0 voice message. `status`
/// is one of [`NOTE_ON`] / [`NOTE_OFF`] / [`CONTROL_CHANGE`]; the code-index
/// nibble (cable 0) is the status high nibble, per the USB-MIDI spec.
fn packet(status: u8, data1: u8, data2: u8) -> [u8; 4] {
    [status >> 4, status, data1, data2]
}

/// Drive the USB-MIDI device from the key matrix until the mode is left.
///
/// Waits for the host to enable the interface, then scans the matrix every
/// millisecond (its own [`matrix::Debouncer`], independent of the keyboard loops
/// that do not run in this mode) and emits a note-on / note-off on each key edge
/// and a control change on the knob edge. Sends are best-effort: a failed write
/// means the host stopped draining (a disconnect or the mode being left), so the
/// loop drops back to waiting for the connection. The caller races this against
/// the mode-change signal, so leaving MIDI mode cancels it cleanly.
pub async fn run<'a>(midi: &mut MidiClass<'a, Driver<'a>>) {
    let mut debouncer = matrix::Debouncer::new();
    // The previous debounced scan; key edges are the diff against it. Starts empty
    // (all released), matching the host's assumption that no note is sounding.
    let mut prev = [0u16; NUM_ROWS];

    loop {
        midi.wait_connection().await;

        loop {
            let scan = debouncer.update(matrix::scan());
            let mut delivered = true;

            for row in 0..NUM_ROWS {
                let changed = scan[row] ^ prev[row];
                if changed == 0 {
                    continue;
                }
                for col in 0..NUM_COLS {
                    let bit = 1u16 << col;
                    if changed & bit == 0 {
                        continue;
                    }
                    let pressed = scan[row] & bit != 0;
                    let pkt = match note_for(row, col) {
                        Some(note) if pressed => packet(NOTE_ON, note, VELOCITY),
                        Some(note) => packet(NOTE_OFF, note, 0),
                        // The knob: full-scale CC while held, zero on release.
                        None if pressed => packet(CONTROL_CHANGE, KNOB_CC, 127),
                        None => packet(CONTROL_CHANGE, KNOB_CC, 0),
                    };
                    if midi.write_packet(&pkt).await.is_err() {
                        // Host stopped draining (disconnect / mode left). Stop
                        // emitting and re-wait; `prev` is committed below only for
                        // the edges that actually went out.
                        delivered = false;
                        break;
                    }
                    // Commit this key's new level so the edge is not re-sent.
                    prev[row] = (prev[row] & !bit) | (scan[row] & bit);
                }
                if !delivered {
                    break;
                }
            }

            if !delivered {
                break;
            }
            Timer::after(Duration::from_millis(1)).await;
        }
    }
}
