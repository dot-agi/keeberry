// SPDX-License-Identifier: GPL-2.0-or-later
//! Default keymap and the layer/report engine.
//!
//! The engine is a deterministic per-scan step over explicit state:
//! [`compute_report`] takes one debounced scan plus the carried [`LayerState`],
//! updates that state in place, and returns the keyboard HID report to send.
//! Threading the layer state explicitly (rather than hiding it) keeps the layer
//! and transparency logic easy to reason about and to test off-target.
//!
//! # Default layout
//!
//! [`DEFAULT_KEYMAP`] is keeberry's power-on 75% ANSI layout for the board: a
//! populated Base layer (layer 0) and Fn layer (layer 1) over fourteen transparent
//! spare layers. The `(row, col)` of every key follows the physical matrix that
//! [`crate::matrix`] scans, so positions line up with the hardware.
//!
//! Key kinds in the tables below:
//!
//! * a basic HID keyboard key (`KEY_A`, `ESCAPE`, ...) -> the same usage;
//! * a modifier (`SHIFT_LEFT`, `META_LEFT` = left GUI, ...) -> the same usage;
//! * the Fn key on the Base layer -> [`momentary_layer(1)`](crate::keycode::momentary_layer);
//! * an unbound-but-present key -> [`TRANSPARENT`] (falls through to the layer below);
//! * a position with no key bound, and every matrix hole -> [`NONE`].
//!
//! The consumer-page media/volume/launch keys (`AUDIO_VOLUME_MUTE`, `AUDIO_VOLUME_UP`, `MEDIA_PLAY_PAUSE`,
//! `LAUNCH_APP2`, ...) carry a [`KeyAction::Consumer`] usage and are emitted over a
//! dedicated consumer-control HID interface (see [`crate::usb`]):
//! [`compute_report`] ignores them — they have no keyboard usage — and
//! [`consumer_usage`] resolves them instead. The base-layer rotary-encoder press
//! (`(0, 14)`) is `AUDIO_VOLUME_MUTE`, a consumer key like the Fn-layer media keys.
//!
//! [`BOOTLOADER`] sits directly on the Fn layer at `(4, 12)`: that key resets into the
//! wb32-dfu bootloader, so DFU entry needs no dedicated boot sublayer.
//!
//! Several matrix positions carry no keymap keycode and stay [`NONE`] because keeberry
//! drives those functions — the wireless/radio controls, the RGB controls, the rotary
//! encoder, and host commands such as EEPROM-clear and NKRO-toggle — over its kcp
//! configuration groups, not keymap keycodes, so a keymap binding there would be inert.
//! The Fn-layer table below records which function owns each such position.

use core::cell::RefCell;
use core::sync::atomic::{AtomicU16, AtomicU32, AtomicU8, Ordering};

use crate::behavior;
use crate::features;
use crate::keycode::*;
use crate::matrix::{NUM_COLS, NUM_ROWS};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::Instant;
use usbd_hid::descriptor::KeyboardReport;

/// Number of keymap layers: base (0), Fn (1), and fourteen spare layers (2..=15)
/// that power on fully transparent for the configurator to fill.
///
/// Sixteen layers cost `LAYERS * NUM_ROWS * NUM_COLS * 2 = 2880 B` of keymap RAM (and
/// the same in the config blob); the active-layer mask is a `u16`, so sixteen layers
/// fill it exactly — its upper bound, one bit per layer.
pub const LAYERS: usize = 16;

/// A full keymap: `[layer][row][col]` of [`Keycode`]. Both the power-on
/// [`DEFAULT_KEYMAP`] and the live RAM store have this shape, and it is the unit
/// the persistence layer ([`crate::config`]) serialises and restores.
pub type Keymap = [[[Keycode; NUM_COLS]; NUM_ROWS]; LAYERS];

/// A fully transparent spare layer: `TRANSPARENT` everywhere a key exists, `NONE` at
/// the seven matrix holes (`[3,12]`, `[4,1]`, `[5,3]`, `[5,4]`, `[5,5]`, `[5,7]`,
/// `[5,8]`). Seeds layers 2..=15 of [`DEFAULT_KEYMAP`] so a held switch into any of
/// them falls straight through to the layers below until the configurator binds keys
/// — defined once rather than repeated per layer.
#[rustfmt::skip]
const SPARE_LAYER: [[Keycode; NUM_COLS]; NUM_ROWS] = [
    [TRANSPARENT,  TRANSPARENT, TRANSPARENT, TRANSPARENT,    TRANSPARENT,  TRANSPARENT,  TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       TRANSPARENT,       TRANSPARENT,        TRANSPARENT,   TRANSPARENT,   TRANSPARENT,  TRANSPARENT],
    [TRANSPARENT,  TRANSPARENT, TRANSPARENT, TRANSPARENT,    TRANSPARENT,  TRANSPARENT,  TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       TRANSPARENT,       TRANSPARENT,        TRANSPARENT,   TRANSPARENT,   TRANSPARENT,  TRANSPARENT],
    [TRANSPARENT,  TRANSPARENT, TRANSPARENT, TRANSPARENT,    TRANSPARENT,  TRANSPARENT,  TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       TRANSPARENT,       TRANSPARENT,        TRANSPARENT,   TRANSPARENT,   TRANSPARENT,  TRANSPARENT],
    [TRANSPARENT,  TRANSPARENT, TRANSPARENT, TRANSPARENT,    TRANSPARENT,  TRANSPARENT,  TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       TRANSPARENT,       TRANSPARENT,        TRANSPARENT,   NONE,          TRANSPARENT,  TRANSPARENT],
    [TRANSPARENT,  NONE,        TRANSPARENT, TRANSPARENT,    TRANSPARENT,  TRANSPARENT,  TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       TRANSPARENT,       TRANSPARENT,        TRANSPARENT,   TRANSPARENT,   TRANSPARENT,  TRANSPARENT],
    [TRANSPARENT,  TRANSPARENT, TRANSPARENT, NONE,           NONE,         NONE,         TRANSPARENT,      NONE,                 NONE,              TRANSPARENT,       TRANSPARENT,        TRANSPARENT,   TRANSPARENT,   TRANSPARENT,  TRANSPARENT],
];

/// The default keymap, indexed `[layer][row][col]`.
///
/// This is the power-on layout. It seeds the mutable RAM keymap [`KEYMAP`] at
/// startup; live edits from the configuration protocol mutate that RAM copy,
/// never this table.
///
/// Rows and columns follow [`crate::matrix`]: a `NONE` entry is either an
/// unbound key or a matrix position with no physical key (the holes at
/// `[3,12]`, `[4,1]`, `[5,3]`, `[5,4]`, `[5,5]`, `[5,7]`, `[5,8]`).
#[rustfmt::skip]
pub const DEFAULT_KEYMAP: Keymap = [
    // ----------------------------------------------------------------------
    // Layer 0 — Base. 75% ANSI. The right-hand column carries the rotary-encoder
    // press and the wireless/radio keys. The knob press [0,14] is AUDIO_VOLUME_MUTE,
    // a consumer key (kept). The wireless keys are driven over kcp, not the keymap, so
    // they are `NONE` here:
    //   [1,14] Bluetooth profile  [2,14] 2.4 GHz  [3,14] USB mode  [4,14] battery query
    // ----------------------------------------------------------------------
    [
        [ESCAPE,       F1,          F2,          F3,             F4,           F5,           F6,               F7,                   F8,                F9,                F10,                F11,           F12,           PRINT_SCREEN, AUDIO_VOLUME_MUTE],
        [BACKQUOTE,    DIGIT_1,     DIGIT_2,     DIGIT_3,        DIGIT_4,      DIGIT_5,      DIGIT_6,          DIGIT_7,              DIGIT_8,           DIGIT_9,           DIGIT_0,            MINUS,         EQUAL,         BACKSPACE,    NONE],
        [TAB,          KEY_Q,       KEY_W,       KEY_E,          KEY_R,        KEY_T,        KEY_Y,            KEY_U,                KEY_I,             KEY_O,             KEY_P,              BRACKET_LEFT,  BRACKET_RIGHT, BACKSLASH,    NONE],
        [CAPS_LOCK,    KEY_A,       KEY_S,       KEY_D,          KEY_F,        KEY_G,        KEY_H,            KEY_J,                KEY_K,             KEY_L,             SEMICOLON,          QUOTE,         NONE,          ENTER,        NONE],
        [SHIFT_LEFT,   NONE,        KEY_Z,       KEY_X,          KEY_C,        KEY_V,        KEY_B,            KEY_N,                KEY_M,             COMMA,             PERIOD,             SLASH,         SHIFT_RIGHT,   ARROW_UP,     NONE],
        [CONTROL_LEFT, META_LEFT,   ALT_LEFT,    NONE,           NONE,         NONE,         SPACE,            NONE,                 NONE,              ALT_RIGHT,         momentary_layer(1), CONTROL_RIGHT, ARROW_LEFT,    ARROW_DOWN,   ARROW_RIGHT],
    ],
    // ----------------------------------------------------------------------
    // Layer 1 — Fn. Transparent (`TRANSPARENT`) wherever Fn keeps the base key, so
    // held-Fn falls through to the base binding. Bound in this build:
    //   * keyboard:  [2,8] INSERT, [2,10] PRINT_SCREEN.
    //   * consumer-control (usage page 0x0C, sent on the consumer HID interface
    //     built in the `usb` module — see `consumer_usage`):
    //       row0 [0,1] LAUNCH_APP1 [0,2] LAUNCH_MAIL [0,3] BROWSER_SEARCH [0,4] BROWSER_HOME
    //            [0,5] MEDIA_SELECT [0,6] MEDIA_PLAY_PAUSE [0,7] MEDIA_TRACK_PREVIOUS [0,8] MEDIA_TRACK_NEXT
    //       row4 [4,4] LAUNCH_APP2 [4,8] AUDIO_VOLUME_MUTE [4,9] AUDIO_VOLUME_DOWN [4,10] AUDIO_VOLUME_UP
    //   * bootloader: [4,12] BOOTLOADER — a direct DFU-entry key, so DFU entry needs no
    //     dedicated boot sublayer.
    // The remaining Fn positions have no host keycode in this build and stay `NONE`:
    // keeberry drives these functions over its kcp configuration groups, not the keymap,
    // so a keymap binding would be inert. The function-bearing positions, by group:
    //   RGB lighting:         [0,13], [1,11], [1,12], [3,9], [4,13], [5,12], [5,13], [5,14]
    //   wireless / radio:     [2,3], [2,4], [2,5] (Bluetooth profiles), [2,6] (2.4 GHz),
    //                         [2,7] (USB mode), [5,6] (battery query)
    //   rotary encoder:       [3,5]
    //   host commands:        [1,0] (EEPROM clear), [4,6] (NKRO toggle), [5,1] (GUI lock)
    //   other board controls: [2,2], [5,0], [5,11]
    // ----------------------------------------------------------------------
    [
        [TRANSPARENT,  LAUNCH_APP1, LAUNCH_MAIL, BROWSER_SEARCH, BROWSER_HOME, MEDIA_SELECT, MEDIA_PLAY_PAUSE, MEDIA_TRACK_PREVIOUS, MEDIA_TRACK_NEXT,  NONE,              NONE,               NONE,          NONE,          NONE,         TRANSPARENT],
        [NONE,         TRANSPARENT, TRANSPARENT, TRANSPARENT,    TRANSPARENT,  TRANSPARENT,  TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       TRANSPARENT,       TRANSPARENT,        NONE,          NONE,          TRANSPARENT,  TRANSPARENT],
        [TRANSPARENT,  TRANSPARENT, NONE,        NONE,           NONE,         NONE,         NONE,             NONE,                 INSERT,            TRANSPARENT,       PRINT_SCREEN,       TRANSPARENT,   TRANSPARENT,   TRANSPARENT,  TRANSPARENT],
        [TRANSPARENT,  TRANSPARENT, TRANSPARENT, TRANSPARENT,    TRANSPARENT,  NONE,         TRANSPARENT,      TRANSPARENT,          TRANSPARENT,       NONE,              TRANSPARENT,        TRANSPARENT,   NONE,          TRANSPARENT,  TRANSPARENT],
        [TRANSPARENT,  NONE,        TRANSPARENT, TRANSPARENT,    LAUNCH_APP2,  TRANSPARENT,  NONE,             TRANSPARENT,          AUDIO_VOLUME_MUTE, AUDIO_VOLUME_DOWN, AUDIO_VOLUME_UP,    TRANSPARENT,   BOOTLOADER,    NONE,         TRANSPARENT],
        [NONE,         NONE,        TRANSPARENT, NONE,           NONE,         NONE,         NONE,             NONE,                 NONE,              TRANSPARENT,       TRANSPARENT,        NONE,          NONE,          NONE,         NONE],
    ],
    // ----------------------------------------------------------------------
    // Layers 2..=15 — spare. Each is [`SPARE_LAYER`]: fully transparent (`TRANSPARENT`)
    // so a held switch into any of them falls straight through to the layers below
    // until the configurator binds keys here; the seven matrix holes stay `NONE`
    // like the layers above.
    // ----------------------------------------------------------------------
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
    SPARE_LAYER,
];

/// The live, editable keymap, seeded from [`DEFAULT_KEYMAP`].
///
/// Held in RAM behind a blocking [`Mutex`] so the configuration protocol can
/// rebind keys at runtime: [`compute_report`] (via [`resolve`]) reads it every
/// scan, while [`set_keycode`] writes it from the kcp KEYMAP group, and both
/// changes take effect on the very next scan.
///
/// # Why a blocking mutex + `RefCell`, and why the borrows are sound
///
/// The blocking [`Mutex`] is const-constructible, so the table lives in `.data`
/// (a single copy initialised from [`DEFAULT_KEYMAP`]); the inner [`RefCell`]
/// supplies the interior mutability the writer needs through a `&` lock guard.
/// Every access — read or write — is a *synchronous* critical section
/// ([`Mutex::lock`]) holding the `RefCell` borrow for only the few instructions
/// it takes to copy a [`Keycode`]; no `.await` is ever held across the borrow.
///
/// The reader ([`keyboard_loop`](crate::usb)) and the writer
/// ([`kcp_loop`](crate::usb)) are two futures on the *same* cooperative
/// thread-mode executor, which can only switch tasks at an `.await`. Because no
/// borrow spans an `.await`, a read and a write can never be live at once, so
/// the `RefCell` is never borrowed re-entrantly and its runtime check cannot
/// panic. The [`CriticalSectionRawMutex`] additionally locks out interrupt-
/// context access, keeping the store sound even once an ISR or a second
/// executor might touch it.
static KEYMAP: Mutex<CriticalSectionRawMutex, RefCell<Keymap>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new(DEFAULT_KEYMAP));

/// Number of tap-hold cells — mod-tap, layer-tap or Space-Cadet — currently bound in
/// [`KEYMAP`].
///
/// These are keymap keycodes, not a [`crate::timed`] engine table, so the timed engine
/// has no cheap table to gate on. This count — maintained by
/// [`set_keycode`]/[`load_into_ram`] as the keymap changes — is that gate:
/// [`tap_hold_present`] is a single relaxed load the timed feature ORs into its
/// `active()` so a keymap with any tap-hold key keeps the engine running (and a
/// tap-hold-free keymap keeps the idle fast path), without scanning the keymap each
/// scan. [`DEFAULT_KEYMAP`] binds none, so the power-on count is zero.
static TAP_HOLD_COUNT: AtomicU16 = AtomicU16::new(0);

/// Whether `kc` is a tap-hold keycode the timed engine must observe even when no
/// other behaviour is configured: mod-tap / layer-tap, and Space-Cadet (which rides
/// the same tap-hold engine — a tap emits a paren / Enter, a hold asserts the modifier).
fn is_tap_hold(kc: Keycode) -> bool {
    matches!(
        kc.classify(),
        KeyAction::ModTap { .. } | KeyAction::LayerTap { .. } | KeyAction::SpaceCadet(_)
    )
}

/// Whether the live keymap binds any tap-hold key — mod-tap, layer-tap or Space-Cadet
/// (a single relaxed load). The timed engine's fast-path gate for the tap-hold decision
/// — see [`TAP_HOLD_COUNT`].
pub fn tap_hold_present() -> bool {
    TAP_HOLD_COUNT.load(Ordering::Relaxed) != 0
}

/// The persistent default (base) layer a `DF(n)` key selects. [`compute_report`] seeds
/// the active mask with `1 << DEFAULT_LAYER` instead of a hard-coded layer 0, so `DF`
/// moves the base every higher layer falls through to. A single relaxed atomic — read
/// twice per scan, written only on a `DF` press or a config restore — so the hot path
/// never locks for it. The config blob persists it across reboots.
static DEFAULT_LAYER: AtomicU8 = AtomicU8::new(0);

/// Packed tri-layer configuration: when layers `l1` and `l2` are both active, layer
/// `l3` is auto-activated (QMK's tri-layer "adjust" pattern). Packed into one atomic
/// for a lock-free read inside [`active_mask`]: byte 0 = enabled (`!= 0`), byte 1 =
/// `l1`, byte 2 = `l2`, byte 3 = `l3`. Disabled (`0`) at power-on; set over kcp and
/// restored from the config blob.
static TRI_LAYER: AtomicU32 = AtomicU32::new(0);

/// The persistent default (base) layer (`DF`).
pub fn default_layer() -> u8 {
    DEFAULT_LAYER.load(Ordering::Relaxed)
}

/// Whether `layer` is a valid default (base) layer (`< LAYERS`). Pulled out so a
/// caller setting several layer fields at once (the kcp `SET_LAYER_CONFIG` op) can
/// validate every field *before* committing any, keeping that write atomic.
pub fn default_layer_valid(layer: u8) -> bool {
    usize::from(layer) < LAYERS
}

/// Set the persistent default (base) layer. Returns `false`, changing nothing, when
/// `layer >= LAYERS` (an out-of-range base would resolve nothing); the `DF(n)` press
/// path and the kcp / config restore both go through here, so the stored value is
/// always in range and [`compute_report`]'s `1 << layer` seed never overflows.
pub fn set_default_layer(layer: u8) -> bool {
    if !default_layer_valid(layer) {
        return false;
    }
    DEFAULT_LAYER.store(layer, Ordering::Relaxed);
    true
}

/// The tri-layer configuration as `(enabled, l1, l2, l3)`.
pub fn tri_layer() -> (bool, u8, u8, u8) {
    let v = TRI_LAYER.load(Ordering::Relaxed);
    (
        v & 0xFF != 0,
        (v >> 8) as u8,
        (v >> 16) as u8,
        (v >> 24) as u8,
    )
}

/// Whether the tri-layer tuple is valid: a disabled rule always is; an enabled one
/// needs every layer in range and `l1 != l2` (two distinct trigger layers). Pulled
/// out so `SET_LAYER_CONFIG` can validate before committing, keeping the write atomic.
pub fn tri_layer_valid(enabled: bool, l1: u8, l2: u8, l3: u8) -> bool {
    !enabled
        || (usize::from(l1) < LAYERS
            && usize::from(l2) < LAYERS
            && usize::from(l3) < LAYERS
            && l1 != l2)
}

/// Set the tri-layer configuration. Returns `false`, changing nothing, when enabling
/// with an out-of-range layer or with `l1 == l2` (a tri-layer needs two distinct
/// trigger layers). Disabling always succeeds and clears the stored layers.
pub fn set_tri_layer(enabled: bool, l1: u8, l2: u8, l3: u8) -> bool {
    if !tri_layer_valid(enabled, l1, l2, l3) {
        return false;
    }
    let packed = if enabled {
        1 | (u32::from(l1) << 8) | (u32::from(l2) << 16) | (u32::from(l3) << 24)
    } else {
        0
    };
    TRI_LAYER.store(packed, Ordering::Relaxed);
    true
}

/// Reset the layer configuration to the firmware default — base layer 0, tri-layer
/// off. Used by the config defaults reset ([`crate::config::reset_to_defaults`]).
pub fn reset_layer_config() {
    DEFAULT_LAYER.store(0, Ordering::Relaxed);
    TRI_LAYER.store(0, Ordering::Relaxed);
}

/// Read the keycode bound at `(layer, row, col)` in the live [`KEYMAP`].
///
/// Returns [`None`] when any index is out of range (`layer >= LAYERS`,
/// `row >= NUM_ROWS`, or `col >= NUM_COLS`) — the bounds the kcp KEYMAP group
/// validates host requests against. The mutex/`RefCell` borrow is synchronous
/// and released before the value is returned.
pub fn get_keycode(layer: usize, row: usize, col: usize) -> Option<Keycode> {
    if layer >= LAYERS || row >= NUM_ROWS || col >= NUM_COLS {
        return None;
    }
    Some(KEYMAP.lock(|k| k.borrow()[layer][row][col]))
}

/// Bind `kc` at `(layer, row, col)` in the live [`KEYMAP`]; the change is picked
/// up by the next [`compute_report`] scan.
///
/// Returns `false`, writing nothing, when any index is out of range (the same
/// bounds as [`get_keycode`]). The write is a synchronous mutex critical section
/// holding the `RefCell` borrow only long enough to store the [`Keycode`].
pub fn set_keycode(layer: usize, row: usize, col: usize, kc: Keycode) -> bool {
    if layer >= LAYERS || row >= NUM_ROWS || col >= NUM_COLS {
        return false;
    }
    // Swap the cell under the lock and take the old value, then adjust the tap-hold
    // count by the delta (so the gate stays O(1) per edit, never a full keymap scan).
    let old = KEYMAP.lock(|k| {
        let cell = &mut k.borrow_mut()[layer][row][col];
        let old = *cell;
        *cell = kc;
        old
    });
    match (is_tap_hold(old), is_tap_hold(kc)) {
        (false, true) => {
            TAP_HOLD_COUNT.fetch_add(1, Ordering::Relaxed);
        }
        (true, false) => {
            TAP_HOLD_COUNT.fetch_sub(1, Ordering::Relaxed);
        }
        _ => {}
    }
    true
}

/// Take a copy of the entire live [`KEYMAP`].
///
/// Used by the persistence layer ([`crate::config`]) to serialise the current
/// bindings for a flash save. The copy is made inside a single synchronous
/// mutex critical section, so it is a consistent snapshot even while scans run.
pub fn snapshot() -> Keymap {
    KEYMAP.lock(|k| *k.borrow())
}

/// Overwrite the entire live [`KEYMAP`] with `km`; the change is picked up by the
/// next [`compute_report`] scan.
///
/// This is how a flash-restored keymap is applied at boot ([`crate::config`]) and
/// how the kcp CONFIG group resets to [`DEFAULT_KEYMAP`]. The store is a single
/// synchronous mutex critical section holding the `RefCell` borrow only long
/// enough to move the table in.
pub fn load_into_ram(km: Keymap) {
    // Recompute the tap-hold gate from the whole table: a bulk replace (boot restore
    // or reset-to-defaults) can change any number of cells, so the per-edit delta
    // [`set_keycode`] keeps does not apply here.
    let count = km
        .iter()
        .flatten()
        .flatten()
        .filter(|&&kc| is_tap_hold(kc))
        .count();
    KEYMAP.lock(|k| *k.borrow_mut() = km);
    TAP_HOLD_COUNT.store(count as u16, Ordering::Relaxed);
}

/// HID usage that fills every slot on n-key rollover overflow
/// (`ErrorRollOver`, usage page `0x07`). Shared with [`crate::timed`]'s overlay
/// merge so both rollover-overflow paths agree on one value.
pub(crate) const ERROR_ROLL_OVER: u8 = 0x01;

/// Number of bytes in the NKRO key bitmap built by [`compute_report`].
///
/// 14 bytes = 112 bits cover HID keyboard usages `0x00..=0x6F`, which is the exact
/// range the vendor wireless NKRO frame carries
/// (`MD_SND_CMD_NKRO_LEN`, see [`crate::wireless::md_send_nkro`]); sizing the USB
/// bitmap to match lets a single bitmap feed both transports byte-for-byte. The
/// whole basic-usage range the default keymap resolves into a report (max `0x52`,
/// `ARROW_UP`) sits well inside it; held usages above `0x6F` ride only the 6KRO boot
/// report, matching the vendor's wireless NKRO range exactly.
pub const NKRO_BYTES: usize = 14;

/// Highest HID keyboard usage representable in the [`NKRO_BYTES`] bitmap (`0x6F`).
const NKRO_USAGE_MAX: usize = NKRO_BYTES * 8 - 1;

/// Capacity of [`Report::high`] — the held usages that fall *outside* the bitmap
/// range (`> NKRO_USAGE_MAX`). They can only be carried by the six-slot boot
/// report, so tracking more than six is pointless: a seventh simultaneous
/// out-of-range key cannot be conveyed on either interface regardless.
pub const NKRO_HIGH_CAP: usize = 6;

/// Record a held basic-key `usage` into the NKRO representation.
///
/// If `usage` fits the [`NKRO_BYTES`] bitmap (`0x00..=0x6F`) its bit is set in
/// `bits`; otherwise it is stashed (deduplicated, capped at [`NKRO_HIGH_CAP`]) in
/// `high`, the list of usages the NKRO split must route to the boot report because
/// the bitmap cannot represent them. Beyond the cap an out-of-range usage is
/// dropped — it cannot ride either interface (the boot report is full and the
/// bitmap cannot hold it), the same loss the 6KRO report signals past six keys.
/// Used by both [`compute_report`] and the timed-behaviour overlay (the
/// [`Timed`](crate::timed::Timed) feature's overlay hook) so injected keys are
/// routed identically.
pub(crate) fn nkro_record(bits: &mut [u8; NKRO_BYTES], high: &mut [u8; NKRO_HIGH_CAP], usage: u8) {
    if (usage as usize) <= NKRO_USAGE_MAX {
        bits[usage as usize / 8] |= 1u8 << (usage % 8);
    } else if !high.contains(&usage) {
        if let Some(slot) = high.iter_mut().find(|s| **s == 0) {
            *slot = usage;
        }
    }
}

/// One scan's resolved key state: the 6KRO boot report and the full NKRO key set.
///
/// [`boot`](Self::boot) is the 6KRO boot report — the
/// first six held basic keys in scan order, or `[ErrorRollOver; 6]` past six — so
/// the wired 6KRO path is unchanged when NKRO is disabled. The NKRO key set is the
/// *complete* set of held basic keys, split by representability: usages
/// `0x00..=0x6F` are bits in [`nkro_bits`](Self::nkro_bits), and the rest (which
/// the bitmap cannot hold) are listed in [`high`](Self::high). Together with the
/// shared modifier byte in `boot.modifier` they carry every held key with no
/// six-key cap, so they are the source for the N-key-rollover dual-send. All three
/// are built in one pass from the same post-SOCD/override key set, so they agree.
pub struct Report {
    /// The 6KRO boot keyboard report (EP1 / the wireless boot path).
    pub boot: KeyboardReport,
    /// Held basic keys in the bitmap range as a bitmap: bit `u & 7` of byte `u >> 3`
    /// is set when usage `u` (`0x00..=0x6F`) is held. The modifier is `boot.modifier`.
    pub nkro_bits: [u8; NKRO_BYTES],
    /// Held basic keys *outside* the bitmap range (`> 0x6F`), zero-padded
    /// (`0` = empty). The NKRO split routes these to the boot report — the only
    /// interface that can carry an unrepresentable usage — so a high key is never
    /// dropped under NKRO just because the bitmap cannot hold it.
    pub high: [u8; NKRO_HIGH_CAP],
}

/// Persistent layer state carried across scans.
///
/// Momentary (`MO`) and tap-toggle-hold (`TT`) layers are recomputed from the held
/// matrix every scan, but the edge-triggered switches latch state that must survive
/// between scans: `TO`/`TG`/`TT`-tap fold into [`toggled`](Self::toggled), `OSL`
/// arms [`oneshot`](Self::oneshot), `LAYER_LOCK` folds into [`locked`](Self::locked),
/// and [`prev`](Self::prev) gives the press/release edges those behaviours fire on.
/// [`compute_report`] is the sole mutator.
pub struct LayerState {
    /// Bitmask of active layers this scan; bit `n` set means layer `n` is active.
    /// The base bit comes from [`default_layer`] (bit 0 by default, or bit `n` once
    /// `DF(n)` has moved the persistent base). The output of each scan, exposed via
    /// [`active`](Self::active).
    active: u16,
    /// Latched layers: the toggle (`TG`) set plus any `TT` tap. Bit `n` set means
    /// layer `n` stays active with no key held. Edited only on key edges, so it
    /// persists across scans.
    toggled: u16,
    /// Locked layers (`LAYER_LOCK`): bit `n` set means layer `n` is held active
    /// independently of any key, like [`toggled`](Self::toggled) but driven by the
    /// layer-lock key rather than `TG`/`TT`. A `LAYER_LOCK` press flips the bit of the
    /// highest currently-active layer, so a momentary / one-shot / tap-toggle layer
    /// stays on after its key lifts (press again to unlock). Persists across scans.
    locked: u16,
    /// One-shot layers (`OSL`): bit `n` set means layer `n` is armed. It stays
    /// active from the arming press until the consuming key (the next real key)
    /// releases, so a held key keeps resolving on the one-shot layer for its whole
    /// hold instead of flipping back to the base mid-press.
    oneshot: u16,
    /// Whether the armed one-shot has been consumed (a real key was pressed under
    /// it) and is now just waiting for every key to lift before it clears.
    oneshot_used: bool,
    /// `TT` layers held as of last scan, so a `TT` release edge can be spotted.
    tt_held: u16,
    /// `TT` layers whose hold has already seen another key down — so their release
    /// is a *use* (momentary), not a tap, and must not toggle.
    tt_used: u16,
    /// Previous scan's matrix: the press/release-edge basis for the edge-triggered
    /// behaviours (`TO`/`TG`/`TT`/`OSL`). The level-triggered `MO`/`TT`-hold paths
    /// never consult it.
    prev: [u16; NUM_ROWS],
}

impl LayerState {
    /// Create the initial state: only the base layer active, nothing latched.
    pub const fn new() -> Self {
        Self {
            active: 1,
            toggled: 0,
            locked: 0,
            oneshot: 0,
            oneshot_used: false,
            tt_held: 0,
            tt_used: 0,
            prev: [0; NUM_ROWS],
        }
    }

    /// The current active-layer bitmask (bit `n` = layer `n`). Read each scan by
    /// [`keyboard_loop`](crate::usb) to feed the timed engine and the telemetry
    /// snapshot.
    pub const fn active(&self) -> u16 {
        self.active
    }
}

impl Default for LayerState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolve the effective keycode at `(row, col)` for a given active-layer mask.
///
/// Walks active layers from the highest down to the base, returning the first
/// non-transparent binding. If every active layer is transparent there, the
/// key is unbound ([`Keycode::NO`]).
///
/// The walk runs inside a single synchronous [`KEYMAP`] lock — one critical
/// section per resolved key, with no `.await` held — so it always observes a
/// consistent snapshot of the live keymap even as the config protocol edits it.
fn resolve(active: u16, row: usize, col: usize) -> Keycode {
    KEYMAP.lock(|k| {
        let keymap = k.borrow();
        let mut layer = LAYERS;
        while layer > 0 {
            layer -= 1;
            if active & (1u16 << layer) == 0 {
                continue;
            }
            let kc = keymap[layer][row][col];
            if kc != Keycode::TRNS {
                return kc;
            }
        }
        Keycode::NO
    })
}

/// Resolve the effective keycode at `(row, col)` under the active-layer mask
/// `active`, exactly as [`compute_report`] does internally.
///
/// Exposed so the timed-behaviour engine ([`crate::timed`]) can label a matrix
/// edge with the keycode the position resolves to. Out-of-range indices resolve to
/// [`Keycode::NO`]. Like [`resolve`], one synchronous [`KEYMAP`] lock.
pub fn resolve_keycode(row: usize, col: usize, active: u16) -> Keycode {
    if row >= NUM_ROWS || col >= NUM_COLS {
        return Keycode::NO;
    }
    resolve(active, row, col)
}

/// Which half of the board a physical key sits on — its handedness.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Hand {
    Left,
    Right,
}

/// The hand of matrix column `col`. The board's matrix is wired left-to-right, so a
/// matrix column is the board's physical grid column (it matches `rgb`'s per-LED
/// `LED_COL` for every key that has an LED under it); the board therefore splits at
/// its centre column `NUM_COLS / 2`, left of which is the left hand and the centre
/// column onward the right. Exposed so the tap-hold engine ([`crate::timed`]) can
/// apply chordal hold (bilateral combinations) — keeping a same-hand roll a tap.
pub(crate) const fn key_hand(col: usize) -> Hand {
    if col < NUM_COLS / 2 {
        Hand::Left
    } else {
        Hand::Right
    }
}

/// Highest set layer index in `mask` (the most-significant active layer), or `0`
/// when only the base layer (bit 0) — or nothing — is set.
///
/// `LAYER_LOCK` uses it to pick the layer to lock: the topmost layer in force at the
/// press, so locking captures the held momentary / armed one-shot the user is on.
fn top_layer(mask: u16) -> u8 {
    if mask == 0 {
        return 0;
    }
    (15 - mask.leading_zeros()) as u8
}

/// Fixed-point active-layer mask: `seed` (the base plus any latched/locked/one-shot
/// layers) extended with every layer reached by a held momentary (`MO`) or
/// tap-toggle-hold (`TT`) key.
///
/// Each held position is resolved through transparency under the mask built so
/// far, so a momentary reached on one layer can activate the next; bits are only
/// ever added, so it settles in at most [`LAYERS`] passes. [`compute_report`] calls
/// it twice a scan — once to resolve this scan's press edges under the layers in
/// force when the keys went down, then again after those edges have updated the
/// latched/one-shot state.
fn active_mask(seed: u16, matrix: [u16; NUM_ROWS]) -> u16 {
    // Tri-layer is part of the fixed point: activating `l3` because `l1 && l2` may
    // expose a held `MO` on `l3`, which the next pass then folds in. Read once up
    // front (a single relaxed load) rather than per pass.
    let (tri_on, l1, l2, l3) = tri_layer();
    let tri_pair = if tri_on {
        (1u16 << l1) | (1u16 << l2)
    } else {
        0
    };
    let tri_bit = if tri_on { 1u16 << l3 } else { 0 };
    let mut active = seed;
    for _ in 0..LAYERS {
        let mut next = active;
        for (r, &row_bits) in matrix.iter().enumerate() {
            let mut bits = row_bits;
            while bits != 0 {
                let c = bits.trailing_zeros() as usize;
                bits &= bits - 1;
                match resolve(active, r, c).classify() {
                    KeyAction::Momentary(n) | KeyAction::TapToggle(n) => {
                        if (n as usize) < LAYERS {
                            next |= 1u16 << n;
                        }
                    }
                    _ => {}
                }
            }
        }
        // Tri-layer: both trigger layers active adds the target layer.
        if tri_on && next & tri_pair == tri_pair {
            next |= tri_bit;
        }
        if next == active {
            break;
        }
        active = next;
    }
    active
}

/// HID modifier bits that make `GraveEscape` emit grave/`~` instead of Escape: either
/// Shift (left = bit 1, right = bit 5) or either GUI (left = bit 3, right = bit 7).
/// QMK's grave-escape includes GUI so `Cmd`/`Win` + `` ` `` reaches the host.
const GESC_GRAVE_MODS: u8 = (1 << 1) | (1 << 3) | (1 << 5) | (1 << 7);
/// HID usage `GraveEscape` emits with no qualifying modifier held (Escape).
const GESC_ESCAPE_USAGE: u8 = 0x29;
/// HID usage `GraveEscape` emits while a qualifying modifier is held (Grave; the held
/// Shift turns it into `~` on the host with no extra work here).
const GESC_GRAVE_USAGE: u8 = 0x35;

/// Compute the keyboard HID report for one debounced matrix scan.
///
/// `matrix[r]` has bit `c` set when key `(r, c)` is held (the format
/// [`crate::matrix::Debouncer::update`] returns). The function:
///
/// 1. updates the layer state from this scan's press/release edges (`matrix` vs the
///    previous scan, kept in [`LayerState`]): `TO` selects a layer and clears the
///    rest, `TG` toggles one, `OSL` arms a one-shot, and a `TT` tap toggles on
///    release. The active-layer mask is then [`active_mask`] over the latched and
///    one-shot layers plus every held `MO`/`TT` (momentary) key;
/// 2. resolves each held key into a modifier byte and a [`behavior::KeySet`] of
///    basic keycodes, applying any one-shot layer to the next key and retiring it
///    once that key lifts;
/// 3. runs the report fold ([`features::run_on_report`]) over that set — SOCD
///    cleanup then key overrides, in registry order — which rewrite it in place
///    (each gates on its own ANY flag, so an unconfigured keyboard reaches step 4
///    with the set untouched);
/// 4. finalises the (possibly rewritten) set into the [`Report`]: modifiers in the
///    modifier byte, basic usages in both the six-slot rollover array (the boot
///    report) and the full NKRO bitmap.
///
/// On more than six simultaneous basic keys the boot array is filled with
/// `ErrorRollOver` per the HID 6-key-rollover convention; modifiers are still
/// reported. The NKRO bitmap ([`Report::nkro_bits`]) has no six-key cap, so it
/// records every held usage `0x00..=0x6F` regardless. Both are built in the same
/// pass, so they describe the same key set.
///
/// The active mask is written back into [`LayerState`], along with the latched and
/// one-shot layer state and this scan's matrix (the next call's edge basis).
pub fn compute_report(matrix: [u16; NUM_ROWS], state: &mut LayerState, now: Instant) -> Report {
    // Layers held momentarily by a hold-resolved layer-tap (`LT`), read once up front
    // — before any `KEYMAP` lock — so the timed engine's lock is never nested inside
    // the keymap's. The timed feature's matrix hook already ran this scan, so this
    // reflects this scan's tap/hold decisions; it folds into the active-mask seed
    // exactly like a held `MO`. Zero (one relaxed load) until an `LT` is bound and held.
    let lt_layers = crate::timed::momentary_layers();

    // --- 1. Drive the latching layer switches off this scan's press edges. ---
    // `MO` and `TT`-hold are level-triggered (folded into the active mask below);
    // `TO`/`TG`/`OSL`/`DF` and the `TT` tap are edge-triggered, so they fire on the
    // press transitions `matrix & !prev`. Each edge is resolved under the layers that
    // were in force when the key went down — the persistent default layer plus the
    // latched/one-shot state plus this scan's momentary keys — so a switch placed on a
    // held momentary layer is reached. The base bit is `1 << DF` (not a hard-coded
    // layer 0), so `DF(n)` moves the layer every transparent key falls through to.
    let base_pre = 1u16 << default_layer();
    let active_pre =
        active_mask(base_pre | state.toggled | state.locked | state.oneshot | lt_layers, matrix);
    for (r, &row_bits) in matrix.iter().enumerate() {
        let mut press = row_bits & !state.prev[r];
        while press != 0 {
            let c = press.trailing_zeros() as usize;
            press &= press - 1;
            match resolve(active_pre, r, c).classify() {
                // `TO(n)`: make layer `n` the only active non-base layer, clearing
                // any toggled, locked or one-shot layer.
                KeyAction::ToLayer(n) if (n as usize) < LAYERS => {
                    state.toggled = if n == 0 { 0 } else { 1u16 << n };
                    state.locked = 0;
                    state.oneshot = 0;
                }
                // `TG(n)`: flip whether layer `n` is latched.
                KeyAction::Toggle(n) if (n as usize) < LAYERS => state.toggled ^= 1u16 << n,
                // `OSL(n)`: arm layer `n` for the next key.
                KeyAction::OneShot(n) if (n as usize) < LAYERS => state.oneshot |= 1u16 << n,
                // `DF(n)`: make layer `n` the persistent base. `set_default_layer`
                // ignores an out-of-range layer, so the stored base stays valid.
                KeyAction::DefaultLayer(n) => {
                    set_default_layer(n);
                }
                // `LAYER_LOCK`: lock (or unlock) the highest currently-active layer, so
                // the layer in force at the press — a held momentary, an armed
                // one-shot or an already-locked layer — stays on with no key held.
                // Re-pressing on the locked layer (now the highest active) clears it.
                KeyAction::LayerLock => {
                    let highest = top_layer(active_pre);
                    if highest != 0 && (highest as usize) < LAYERS {
                        state.locked ^= 1u16 << highest;
                    }
                }
                // Auto-shift control keys flip the timed engine's runtime enable
                // flag; they carry no HID usage and are handled here (on the
                // always-run report path) so they work even with nothing else
                // configured. The borrow of the keymap from `resolve` has already
                // been released, so taking the timed lock here nests no mutexes.
                KeyAction::AutoShift(action) => crate::timed::autoshift_control(action),
                // The caps-word / key-lock / repeat / alt-repeat keys carry no HID usage
                // either; like the codes above they are driven on their press edge,
                // dispatching to their `crate::features` plugins (the keymap borrow is
                // already released, so no lock nests). A `#[cfg]`-disabled feature's arm
                // vanishes and its keycode falls through to the no-op default below.
                #[cfg(feature = "caps_word")]
                KeyAction::CapsWord => crate::features::caps_word::CAPS_WORD.engage(),
                #[cfg(feature = "key_lock")]
                KeyAction::KeyLock => crate::features::key_lock::KEY_LOCK.arm(r as u8, c as u8),
                #[cfg(feature = "one_shot_mod")]
                KeyAction::OneShotMod(bit) => crate::features::one_shot_mod::ONE_SHOT_MOD.arm(bit),
                #[cfg(feature = "repeat_key")]
                KeyAction::Repeat => crate::features::repeat_key::REPEAT.repeat(),
                #[cfg(feature = "repeat_key")]
                KeyAction::AltRepeat => crate::features::repeat_key::REPEAT.alt_repeat(),
                #[cfg(feature = "autocorrect")]
                KeyAction::Autocorrect(action) => {
                    crate::features::autocorrect::AUTOCORRECT.control(action)
                }
                // The Unicode keys carry no HID usage either: a press cycles the OS input
                // mode or starts a codepoint send, both driven on the press edge by the
                // unicode plugin (the keymap borrow from `resolve` is already released, so
                // no lock nests). A `#[cfg]`-disabled build drops both arms to the default.
                #[cfg(feature = "unicode")]
                KeyAction::UnicodeCycle => crate::features::unicode::UNICODE.cycle_mode(),
                #[cfg(feature = "unicode")]
                KeyAction::UnicodeMap(slot) => crate::features::unicode::UNICODE.send(slot),
                _ => {}
            }
        }
    }

    // The active mask for this scan's output: the (now updated) default, latched,
    // locked and one-shot layers, extended with every held momentary / tap-toggle key.
    // The base bit is re-read after the edge loop, so a `DF` pressed this scan takes
    // effect immediately.
    let base = 1u16 << default_layer();
    let active = active_mask(base | state.toggled | state.locked | state.oneshot | lt_layers, matrix);
    state.active = active;

    // --- 2. Resolve held keys into a modifier byte and a working key set. ---
    // Modifiers fold into `mods`; every basic key is collected, in scan order, into
    // `keys` so the stateless behaviours can rewrite the set before it is finalised.
    // NoOp, Transparent (resolved away) and the layer switches contribute no usage
    // here — consumer usages go out on their own interface (see [`consumer_usage`]).
    // Two edge facts are gathered alongside: `tt_now` (the layers held by a `TT`
    // key, for the tap/hold decision) and `consume_press` (a real key was newly
    // pressed under the active layers — the keypress that spends a one-shot).
    let mut mods: u8 = 0;
    let mut keys = behavior::KeySet::new();
    let mut tt_now: u16 = 0;
    let mut consume_press = false;
    // A held grave-escape (`GraveEscape`): pushed as an Escape placeholder before the report
    // fold, then rewritten to grave afterwards once `mods` reflects every hook's modifier.
    let mut gesc_held = false;
    for (r, &row_bits) in matrix.iter().enumerate() {
        let press = row_bits & !state.prev[r];
        let mut bits = row_bits;
        while bits != 0 {
            let c = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            let new_press = press & (1u16 << c) != 0;
            let kc = resolve(active, r, c);
            match kc.classify() {
                KeyAction::Modifier(bit) => {
                    mods |= 1u8 << bit;
                    consume_press |= new_press;
                }
                KeyAction::Key(_) => {
                    keys.push(kc);
                    consume_press |= new_press;
                    // Remember the freshly-pressed key so `Repeat` can repeat it; the
                    // modifiers held with it are snapshotted by the plugin's overlay hook,
                    // once this scan's final modifiers are known.
                    #[cfg(feature = "repeat_key")]
                    if new_press {
                        crate::features::repeat_key::REPEAT.record_press(kc);
                    }
                    // A fresh key press is what consumes a pending one-shot modifier: it
                    // latches onto this key (never one held through the arm) and stays only
                    // while this key is held.
                    #[cfg(feature = "one_shot_mod")]
                    if new_press {
                        crate::features::one_shot_mod::ONE_SHOT_MOD.consume_press(kc);
                    }
                    // Autocorrect is fed from the *finished* report below (after the fold),
                    // not here, so its buffer reflects the keys the host actually receives.
                }
                // Consumer / tap-dance / macro / mod-tap / layer-tap keys emit nothing
                // on this path (their output is the consumer interface, or the timed
                // engine's overlay / momentary layer), but a fresh press of one is still
                // the keypress that spends a one-shot.
                KeyAction::Consumer(_)
                | KeyAction::TapDance(_)
                | KeyAction::Macro(_)
                | KeyAction::ModTap { .. }
                | KeyAction::LayerTap { .. }
                | KeyAction::SpaceCadet(_) => {
                    consume_press |= new_press;
                }
                // Grave-escape carries a usage, but which one (Escape vs grave)
                // depends on the final modifier byte, so it is resolved after the
                // fold. A fresh press still spends a one-shot like any key — folding
                // it onto the Escape placeholder, which the post-fold step rewrites to
                // grave if the resulting modifier qualifies (so `OSM(Shift)` + `GraveEscape`
                // is `~`).
                KeyAction::GraveEscape => {
                    gesc_held = true;
                    consume_press |= new_press;
                    #[cfg(feature = "one_shot_mod")]
                    if new_press {
                        crate::features::one_shot_mod::ONE_SHOT_MOD
                            .consume_press(Keycode::from_usage(GESC_ESCAPE_USAGE));
                    }
                }
                // A held `TT` key keeps its layer momentarily active (already folded
                // into `active`); record it so its release can decide tap vs hold.
                KeyAction::TapToggle(n) => {
                    if (n as usize) < LAYERS {
                        tt_now |= 1u16 << n;
                    }
                }
                // A held `BOOTLOADER` jumps into the wb32-dfu bootloader; this never
                // returns (the MCU resets), so no report is produced this scan.
                KeyAction::Boot => crate::boot::bootloader_jump(),
                _ => {}
            }
        }
    }

    // Grave-escape: push a placeholder Escape ahead of the report fold so SOCD, overrides,
    // One-Shot-Mod and Caps Word all see it like any basic key; its final Escape-vs-grave
    // identity is settled *after* the fold (below), once `mods` reflects every hook's
    // modifier. Remember the slot so only this key — not a real Escape the user is also
    // holding — is rewritten.
    let mut gesc_idx = None;
    if gesc_held {
        gesc_idx = Some(keys.as_slice().len());
        keys.push(Keycode::from_usage(GESC_ESCAPE_USAGE));
    }

    // Resolve the tap-toggle releases. A `TT` layer becomes "used" (its hold acted
    // as a momentary) once another key is down alongside it; a release that was
    // never used is a bare tap and toggles the layer.
    let held_count: u32 = matrix.iter().map(|r| r.count_ones()).sum();
    if tt_now != 0 && held_count > 1 {
        state.tt_used |= tt_now;
    }
    let tt_released = state.tt_held & !tt_now;
    state.toggled ^= tt_released & !state.tt_used;
    state.tt_used &= !tt_released;
    state.tt_held = tt_now;

    // Retire a spent one-shot. The consuming keypress marks it used but leaves the
    // layer active; it clears only once every key has lifted, so a held key keeps
    // resolving on the one-shot layer for its whole press rather than reverting to
    // the base after the first scan.
    if state.oneshot != 0 && consume_press {
        state.oneshot_used = true;
    }
    if state.oneshot_used && held_count == 0 {
        state.oneshot = 0;
        state.oneshot_used = false;
    }
    state.prev = matrix;

    // --- 3. The report fold: SOCD cleanup, then key overrides. ---
    // `run_on_report` runs each active feature in registry order (SOCD before
    // overrides, a fixed order); each gates on its own ANY flag, so when both tables
    // are empty (the power-on default) `mods` and `keys` are left exactly as resolved
    // above and the report below is byte-identical to an unconfigured keyboard's. The
    // report fold does not consult `prev_matrix`; it is carried for the matrix fold
    // (driven from the keyboard loop with last scan's matrix), so `state.prev` — this
    // scan's matrix here — is a valid placeholder the current report features ignore.
    let ctx = features::Ctx {
        now,
        active_layers: active,
        prev_matrix: &state.prev,
    };
    features::run_on_report(&ctx, &mut mods, &mut keys);

    // Settle grave-escape now that the fold has folded in every hook's modifier: a held
    // `GraveEscape` emits grave (`` ` ``; the held Shift renders it `~` on the host) while any
    // Shift or GUI survives in the final `mods`, else it stays the placeholder Escape. The
    // slot is rewritten in place, so a real Escape held alongside is untouched.
    if let Some(idx) = gesc_idx {
        if mods & GESC_GRAVE_MODS != 0 {
            keys.replace_at(idx, Keycode::from_usage(GESC_GRAVE_USAGE));
        }
    }

    // --- 4. Finalise the report from the (possibly rewritten) set. ---
    // The boot report is built as the 6KRO boot path does (first six usages in scan
    // order, ErrorRollOver past six); the NKRO bitmap is filled additively in the same
    // walk and never feeds back into the boot report, so a build that ignores
    // `nkro_bits` is byte-for-byte that same 6KRO report.
    let mut boot = KeyboardReport::default();
    boot.modifier = mods;
    let mut nkro_bits = [0u8; NKRO_BYTES];
    let mut high = [0u8; NKRO_HIGH_CAP];
    let mut nkeys = 0usize;
    // A set that overflowed its buffer is already past six basic keys, so the boot
    // report maps straight to a rollover-overflow report.
    let mut overflow = keys.truncated();
    for &kc in keys.as_slice() {
        match kc.classify() {
            KeyAction::Key(usage) => {
                // Boot (6KRO) report: first six usages, ErrorRollOver past six.
                if nkeys < boot.keycodes.len() {
                    boot.keycodes[nkeys] = usage;
                    nkeys += 1;
                } else {
                    overflow = true;
                }
                // NKRO set: record every held usage, no six-key cap. In-range usages
                // set a bitmap bit; out-of-range usages (which the bitmap cannot
                // hold) join `high` for the split to route to the boot report — so
                // a high usage is captured even when the 6KRO `boot` above has
                // already overflowed to ErrorRollOver and lost it.
                nkro_record(&mut nkro_bits, &mut high, usage);
            }
            // An override may rewrite a trigger to a modifier (folds into the
            // modifier byte) or to a suppressed `NONE` slot (emits nothing); a
            // SOCD-suppressed key is likewise a `NONE` no-op here.
            KeyAction::Modifier(bit) => boot.modifier |= 1u8 << bit,
            _ => {}
        }
    }
    if overflow {
        boot.keycodes = [ERROR_ROLL_OVER; 6];
    }
    // Feed the finished report to autocorrect: it diffs the emitted keys (the `nkro_bits`
    // bitmap plus the `high` usage list — exactly what the host receives, after SOCD and key
    // overrides) against last scan's to find the fresh presses, matches the rolling word buffer
    // and arms a correction. Done here, not in the resolve walk, so a letter an override dropped
    // or swapped never miscounts the backspaces; the in-flight gate it sets is read by this same
    // scan's overlay, so a fired correction masks its trigger letter immediately.
    #[cfg(feature = "autocorrect")]
    crate::features::autocorrect::AUTOCORRECT.record_report(&nkro_bits, &high, boot.modifier);
    Report {
        boot,
        nkro_bits,
        high,
    }
}

/// Resolve the consumer-control usage to report for one debounced matrix scan.
///
/// `active` is the layer mask [`compute_report`] derived from the *same* scan
/// (`state.active()` after it runs), so the two reports — keyboard and
/// consumer — always agree on the active layers. The scan is walked in the same
/// order, and the first pressed key that resolves to a [`KeyAction::Consumer`]
/// wins; its 16-bit HID consumer usage (usage page `0x0C`) is returned, or `0`
/// when no consumer key is held.
///
/// Only one usage is reported at a time, which is all the media / volume /
/// brightness / launcher keys need: the host treats a non-zero usage as that
/// key pressed and `0` as released, the single-16-bit-consumer-usage shape the
/// [`crate::usb`] shared interface sends as report 2. As in [`resolve`], each
/// lookup is a single synchronous [`KEYMAP`] critical section.
pub fn consumer_usage(matrix: [u16; NUM_ROWS], active: u16) -> u16 {
    for (r, &row_bits) in matrix.iter().enumerate() {
        let mut bits = row_bits;
        while bits != 0 {
            let c = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if let KeyAction::Consumer(usage) = resolve(active, r, c).classify() {
                return usage;
            }
        }
    }
    0
}

/// Resolve the set of held mouse keys for one debounced matrix scan into the
/// [`crate::mouse`] bitmask.
///
/// `active` is the layer mask [`compute_report`] derived from the *same* scan, so
/// the mouse keys agree with the keyboard report on the active layers. Every held
/// position that resolves to a [`KeyAction::Mouse`] sets its `M_*` bit; unlike
/// [`consumer_usage`] this accumulates *all* held mouse keys (so e.g. holding a
/// movement key while clicking works), since a mouse report carries buttons,
/// movement and wheel together. The result is `0` when no mouse key is held.
///
/// [`crate::usb`] publishes this each scan for its shared-interface loop to feed
/// the [`crate::mouse::Accel`] accelerator and emit mouse HID reports. As in
/// [`resolve`], each lookup is a single synchronous [`KEYMAP`] critical section.
pub fn mouse_keys(matrix: [u16; NUM_ROWS], active: u16) -> u16 {
    let mut keys = 0u16;
    for (r, &row_bits) in matrix.iter().enumerate() {
        let mut bits = row_bits;
        while bits != 0 {
            let c = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if let KeyAction::Mouse(mk) = resolve(active, r, c).classify() {
                keys |= match mk {
                    MouseKey::Up => crate::mouse::M_UP,
                    MouseKey::Down => crate::mouse::M_DOWN,
                    MouseKey::Left => crate::mouse::M_LEFT,
                    MouseKey::Right => crate::mouse::M_RIGHT,
                    MouseKey::Btn1 => crate::mouse::M_BTN1,
                    MouseKey::Btn2 => crate::mouse::M_BTN2,
                    MouseKey::Btn3 => crate::mouse::M_BTN3,
                    MouseKey::WheelUp => crate::mouse::M_WHEEL_UP,
                    MouseKey::WheelDown => crate::mouse::M_WHEEL_DOWN,
                };
            }
        }
    }
    keys
}

/// Resolve the set of held gamepad keys for one debounced matrix scan into the
/// [`crate::gamepad`] bitmask.
///
/// `active` is the layer mask [`compute_report`] derived from the *same* scan, so
/// the gamepad keys agree with the keyboard report on the active layers. Every held
/// position that resolves to a [`KeyAction::Gamepad`] sets its bit; like
/// [`mouse_keys`] this accumulates *all* held gamepad keys, since one gamepad report
/// carries every button and axis together. The result is `0` when none is held.
///
/// [`crate::usb`] publishes this each scan for its shared-interface loop to decode
/// ([`crate::gamepad::buttons`] / [`crate::gamepad::axes`]) and emit gamepad HID
/// reports. As in [`resolve`], each lookup is a single synchronous [`KEYMAP`]
/// critical section.
pub fn gamepad_keys(matrix: [u16; NUM_ROWS], active: u16) -> u32 {
    let mut keys = 0u32;
    for (r, &row_bits) in matrix.iter().enumerate() {
        let mut bits = row_bits;
        while bits != 0 {
            let c = bits.trailing_zeros() as usize;
            bits &= bits - 1;
            if let KeyAction::Gamepad(gk) = resolve(active, r, c).classify() {
                keys |= match gk {
                    // Button `n` (`0..=15`, guaranteed by `classify`) is bit `n` of
                    // the low button field the gamepad report carries.
                    GamepadKey::Button(n) => 1u32 << n,
                    GamepadKey::AxisXNeg => crate::gamepad::X_NEG,
                    GamepadKey::AxisXPos => crate::gamepad::X_POS,
                    GamepadKey::AxisYNeg => crate::gamepad::Y_NEG,
                    GamepadKey::AxisYPos => crate::gamepad::Y_POS,
                    GamepadKey::AxisZNeg => crate::gamepad::Z_NEG,
                    GamepadKey::AxisZPos => crate::gamepad::Z_POS,
                    GamepadKey::AxisRzNeg => crate::gamepad::RZ_NEG,
                    GamepadKey::AxisRzPos => crate::gamepad::RZ_POS,
                };
            }
        }
    }
    keys
}
