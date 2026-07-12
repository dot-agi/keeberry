// SPDX-License-Identifier: GPL-2.0-or-later
//! Compact keycode model for the keymap engine.
//!
//! A [`Keycode`] is a 16-bit newtype. The firmware owns this representation
//! end to end (it is deliberately *not* QMK's or keyberon's) so the kcp
//! configuration protocol defines its own wire format on top of it. The
//! whole model fits the board's 28 KB RAM budget easily: the
//! [`crate::keymap::DEFAULT_KEYMAP`] seed is a flash-resident `const` and a
//! [`Keycode`] is two bytes (the live keymap it seeds lives in RAM).
//!
//! # Encoding
//!
//! The 16-bit space is partitioned into a few disjoint regions. Decoding is a
//! single [`Keycode::classify`] call returning a [`KeyAction`]; nothing else in
//! the engine needs to know the bit layout.
//!
//! | Range             | Meaning                                                                                                     |
//! |-------------------|-------------------------------------------------------------------------------------------------------------|
//! | `0x0000`          | [`NONE`] — no-op (an unbound key)                                                                           |
//! | `0x0001`          | [`TRANSPARENT`] — transparent; fall through to a lower layer                                                |
//! | `0x0004..=0x00DF` | basic HID keyboard usage (usage page `0x07`)                                                                |
//! | `0x00E0..=0x00E7` | the eight modifiers (`LCtrl`..`RGui`), also page `0x07`                                                     |
//! | `0x00E8..=0x00FF` | remaining basic HID usages                                                                                  |
//! | `0x2000..=0x3FFF` | `MT(mods,kc)` — mod-tap: hold `mods`, tap `kc`                                                              |
//! | `0x4000..=0x4FFF` | `LT(layer,kc)` — layer-tap: hold `layer`, tap `kc`                                                          |
//! | `0x5200..=0x521F` | `MO(n)` — momentary layer switch to layer `n` (0..=31)                                                      |
//! | `0x5300..=0x531F` | `TO(n)` — activate layer `n`, others off (0..=31)                                                           |
//! | `0x5400..=0x541F` | `TG(n)` — toggle layer `n` (0..=31)                                                                         |
//! | `0x5500..=0x55FF` | mouse keys (move / buttons / wheel) — see [`MouseKey`]                                                      |
//! | `0x5600..=0x56FF` | gamepad keys (buttons / axes) — see [`GamepadKey`]                                                          |
//! | `0x5700..=0x57FF` | `TD(n)` — tap-dance entry `n` (0..=255)                                                                     |
//! | `0x5800..=0x581F` | `TT(n)` — tap = toggle / hold = momentary (0..=31)                                                          |
//! | `0x5900..=0x591F` | `OSL(n)` — one-shot layer `n` until next key (0..=31)                                                       |
//! | `0x5A00..=0x5A08` | firmware behaviour controls (layer-lock / auto-shift / leader / caps-word / key-lock / repeat / alt-repeat) |
//! | `0x5A09..=0x5A10` | `OSM(mod)` — one-shot modifier; HID modifier bit index in low bits                                          |
//! | `0x5A20..=0x5A2F` | `DF(n)` — set the persistent default layer (0..=15)                                                         |
//! | `0x5A30`          | `GraveEscape` — grave-escape (Esc, or grave/`~` under Shift/GUI)                                            |
//! | `0x5A31..=0x5A33` | Space-Cadet shift keys (paren / Enter on tap, modifier on hold)                                             |
//! | `0x5A40..=0x5A42` | autocorrect controls (`AUTOCORRECT_TOGGLE` / `AUTOCORRECT_ON` / `AUTOCORRECT_OFF`)                          |
//! | `0x5A50`          | `UNICODE_MODE_CYCLE` — cycle the active OS Unicode input mode                                               |
//! | `0x5A51..=0x5A60` | `UM(n)` — emit Unicode-map slot `n`'s codepoint (0..=15)                                                    |
//! | `0x7700..=0x77FF` | `MACRO(n)` — dynamic-macro entry `n` (0..=255)                                                              |
//! | `0x7C00`          | [`BOOTLOADER`] — reset into the wb32-dfu bootloader                                                         |
//! | `0xC000..=0xFFFF` | consumer-control usage (usage page `0x0C`), 14-bit usage                                                    |
//! | everything else   | unassigned, decoded as [`NONE`]                                                                             |
//!
//! Modifiers are not a separate concept from keys: HID usage page `0x07`
//! assigns the eight modifier usages `0xE0..=0xE7`, so a modifier is just a
//! basic usage in that sub-range. [`Keycode::classify`] reports it as a
//! [`KeyAction::Modifier`] carrying the bit index (`usage - 0xE0`), which is
//! exactly the bit it sets in the HID report's modifier byte.
//!
//! The `MO` mnemonic is QMK's name for a momentary layer switch, kept for the
//! engineer's familiarity; the numeric base is the firmware's own, unrelated to
//! QMK's numbering.
//!
//! `TO`/`TG`/`TT`/`OSL` are the other layer-switch families. Each shares the `MO`
//! layout — a `0x20`-code window holding the target layer in the low five bits
//! ([`MO_LAYER_MASK`]) — and mirrors its QMK name: `TO(n)` (`0x5300`) makes layer
//! `n` the only active non-base layer; `TG(n)` (`0x5400`) toggles it; `TT(n)`
//! (`0x5800`) is momentary while held yet toggles on a bare tap; `OSL(n)`
//! (`0x5900`) arms layer `n` for the next key only. Unlike `MO` (recomputed from
//! the held matrix each scan), these latch state that
//! [`crate::keymap::compute_report`] carries across scans.
//!
//! Consumer-control keys (media transport, volume, brightness and the
//! application launchers) occupy the top quarter of the space,
//! `0xC000..=0xFFFF`. The low 14 bits hold the HID consumer usage (usage page
//! `0x0C`); [`Keycode::classify`] reports it as a [`KeyAction::Consumer`]
//! carrying that usage, which [`crate::usb`] emits on a dedicated
//! consumer-control HID interface rather than the keyboard. The region sits
//! far above every other one (`NO`/`TRNS`/basic/modifier are `< 0x0100`, `MO` is
//! `0x52xx`), so the encodings can never collide; 14 bits spans the entire
//! consumer page, well past the largest usage this firmware binds (`0x223`).
//!
//! Tap-dance (`TD(n)`, `0x5700..=0x57FF`) and dynamic macros (`MACRO(n)`,
//! `0x7700..=0x77FF`) name an entry in the RAM tables the timed-behaviour engine
//! ([`crate::timed`]) owns; `n` is the table index (`0..=255`). Both bases mirror
//! QMK's `QK_TAP_DANCE` / `QK_MACRO` for the engineer's familiarity, but the
//! firmware never relies on QMK's numbering. The two 256-code windows sit in the
//! gaps the other regions leave (`MO` ends at `0x521F`, consumer begins at
//! `0xC000`), so they cannot overlap `NO`/`TRNS`/basic/modifier (`< 0x0100`), `MO`
//! (`0x52xx`) or consumer (`>= 0xC000`); a compile-time assertion below pins the
//! disjointness. [`Keycode::classify`] reports them as [`KeyAction::TapDance`] /
//! [`KeyAction::Macro`] carrying that index; [`crate::keymap::compute_report`]
//! emits nothing for them (they have no direct HID usage) — the engine resolves
//! them into real key events out of band, exactly as consumer keys are routed to
//! their own interface.
//!
//! Mouse keys (`0x5500..=0x55FF`) name a [`MouseKey`] — the nine USB HID mouse
//! actions (four-way pointer move, the three buttons, wheel up/down). The region
//! sits in the gap between `MO` (`<= 0x521F`) and `TD` (`>= 0x5700`), pinned by a
//! compile-time assertion below. [`Keycode::classify`] reports them as
//! [`KeyAction::Mouse`], which [`crate::keymap::compute_report`] emits nothing for
//! (they carry no keyboard usage); the [`crate::mouse`] accelerator turns the held
//! mouse keys into mouse HID reports on the shared interface ([`crate::usb`]),
//! exactly as consumer keys are routed to their own report.
//!
//! Gamepad keys (`0x5600..=0x56FF`) name a [`GamepadKey`] — sixteen momentary
//! buttons (`0x5600..=0x560F`) and eight axis-direction keys (`0x5610..=0x5617`)
//! that drive the four signed axes of a DirectInput-style joystick. The region sits
//! in the gap between mouse (`<= 0x55FF`) and `TD` (`>= 0x5700`), pinned by a
//! compile-time assertion below. Like mouse keys these carry no keyboard usage, so
//! [`Keycode::classify`] reports them as [`KeyAction::Gamepad`] and
//! [`crate::keymap::compute_report`] emits nothing for them; the [`crate::gamepad`]
//! decoder turns the held gamepad keys into gamepad HID reports on the shared
//! interface ([`crate::usb`]).
//!
//! Mod-tap (`MT`, `0x2000..=0x3FFF`) and layer-tap (`LT`, `0x4000..=0x4FFF`) are the
//! tap-hold dual-role keys: a tap emits a basic key, a hold applies modifiers (`MT`)
//! or activates a layer (`LT`). Both sit in the otherwise-unassigned span below `MO`
//! (`0x0100..=0x51FF`), at QMK's `MT`/`LT` bases for the engineer's familiarity; a
//! compile-time assertion below pins them disjoint from the basic usages (`< 0x0100`),
//! from each other and from `MO`. The low byte holds the tap usage `kc`; `MT` packs a
//! 5-bit QMK-style modifier selector in bits `8..=12` (bit `12` = right side, bits
//! `8..=11` = Ctrl/Shift/Alt/GUI) which [`Keycode::classify`] expands into the HID
//! modifier byte, and `LT` packs a 4-bit layer in bits `8..=11` (layers `0..=15`).
//! Like `TD`/`MACRO` they carry no direct HID usage — [`KeyAction::ModTap`] /
//! [`KeyAction::LayerTap`] are resolved out of band by the [`crate::timed`] engine,
//! which owns the tap-vs-hold decision and its tuning; [`crate::keymap::compute_report`]
//! emits nothing for them.
//!
//! Firmware behaviour controls (`0x5A00..=0x5A08`) are the parameterless control
//! codes the keymap and timed engines act on directly rather than emitting: layer
//! lock ([`KeyAction::LayerLock`]), the three auto-shift toggles
//! ([`KeyAction::AutoShift`]), the leader trigger ([`KeyAction::Leader`]) and the four
//! behaviour keys — caps-word ([`KeyAction::CapsWord`]), key-lock
//! ([`KeyAction::KeyLock`]) and the two repeat keys ([`KeyAction::Repeat`] /
//! [`KeyAction::AltRepeat`]). One-shot-modifier keys ([`KeyAction::OneShotMod`]) follow
//! at `0x5A09..=0x5A10`, the HID modifier bit index (`0..=7`) in the low bits. Like
//! [`KeyAction::Boot`] they carry no HID usage, so [`crate::keymap::compute_report`]
//! emits nothing for them; it acts on the layer-lock and auto-shift codes itself, the
//! leader code is consumed by the [`crate::timed`] sequence engine, and the
//! caps-word / key-lock / repeat / alt-repeat keys drive their [`crate::features`] plugins
//! on the press edge. The whole region sits in
//! the gap between `OSL` (`<= 0x591F`) and `MACRO` (`>= 0x7700`), pinned by compile-time
//! assertions below.

// This module is the firmware's keycode model: it defines the whole basic HID
// keyboard page and the engine helpers, of which the default keymap uses only a
// subset. The rest is deliberate API surface for richer keymaps and the kcp
// configuration protocol, so unused entries are expected.
#![allow(dead_code)]

/// A single keycode. See the [module documentation](self) for the encoding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Keycode(u16);

/// Decoded meaning of a [`Keycode`], produced by [`Keycode::classify`].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyAction {
    /// No-op: an unbound key. Emits nothing.
    NoOp,
    /// Transparent: defer to the next active layer below this one.
    Transparent,
    /// A basic key, carrying its HID keyboard usage (usage page `0x07`).
    Key(u8),
    /// A modifier, carrying its bit index `0..=7` within the HID modifier byte
    /// (`0` = `LCtrl`, `1` = `LShift`, .., `7` = `RGui`).
    Modifier(u8),
    /// Momentary layer switch, carrying the target layer index.
    Momentary(u8),
    /// Activate-layer switch (`TO`), carrying the target layer index. On press it
    /// makes that layer the only active non-base layer, clearing any other
    /// toggled or one-shot layer.
    ToLayer(u8),
    /// Toggle-layer switch (`TG`), carrying the target layer index. Each press
    /// flips whether that layer is latched active.
    Toggle(u8),
    /// Tap-or-hold layer switch (`TT`), carrying the target layer index. Held, it
    /// is momentary; a bare tap (released with no other key pressed meanwhile)
    /// toggles the layer like [`Toggle`](Self::Toggle).
    TapToggle(u8),
    /// One-shot layer switch (`OSL`), carrying the target layer index. It stays
    /// active only until the next key press, which it applies before clearing.
    OneShot(u8),
    /// Mod-tap (`MT`): tap emits the basic key `kc`, hold asserts `mods` (the HID
    /// modifier byte, expanded from the keycode's 5-bit selector). It carries no
    /// direct HID usage: the [`crate::timed`] engine decides tap vs hold and emits
    /// the tap as a key event or holds the modifiers.
    ModTap {
        /// HID modifier byte held while the key is held past the tapping term.
        mods: u8,
        /// Basic HID keyboard usage emitted on a tap.
        kc: u8,
    },
    /// Layer-tap (`LT`): tap emits the basic key `kc`, hold momentarily activates
    /// `layer`. Like [`ModTap`](Self::ModTap) it carries no direct HID usage; the
    /// [`crate::timed`] engine decides tap vs hold, and the held layer is folded into
    /// [`crate::keymap::compute_report`]'s active-layer mask.
    LayerTap {
        /// Layer activated while the key is held past the tapping term.
        layer: u8,
        /// Basic HID keyboard usage emitted on a tap.
        kc: u8,
    },
    /// A consumer-control key, carrying its 16-bit HID consumer usage (usage
    /// page `0x0C`) to emit on the consumer-control interface.
    Consumer(u16),
    /// A tap-dance key, carrying its entry index into the timed engine's
    /// tap-dance table ([`crate::timed`]). It has no direct HID usage: the engine
    /// resolves it (tap / hold / double-tap) into real key events.
    TapDance(u8),
    /// A dynamic-macro key, carrying its entry index into the timed engine's macro
    /// table ([`crate::timed`]). Pressing it triggers playback of that macro's
    /// recorded event sequence; it emits no HID usage of its own.
    Macro(u8),
    /// A mouse key, carrying the [`MouseKey`] action it performs. It has no HID
    /// keyboard usage: [`crate::keymap::compute_report`] emits nothing for it, and
    /// the [`crate::mouse`] accelerator turns the held mouse keys into mouse HID
    /// reports on the shared interface (see [`crate::usb`]).
    Mouse(MouseKey),
    /// A gamepad key, carrying the [`GamepadKey`] action it performs. Like a mouse
    /// key it has no HID keyboard usage: [`crate::keymap::compute_report`] emits
    /// nothing for it, and the [`crate::gamepad`] decoder turns the held gamepad
    /// keys into gamepad HID reports on the shared interface (see [`crate::usb`]).
    Gamepad(GamepadKey),
    /// The bootloader-entry key, `BOOTLOADER`. It carries no HID usage: when
    /// [`crate::keymap::compute_report`] resolves a held key to this it jumps into
    /// the `wb32-dfu` bootloader ([`crate::boot::bootloader_jump`]).
    Boot,
    /// Layer-lock key (`LAYER_LOCK`). It carries no HID usage: on press
    /// [`crate::keymap::compute_report`] locks the highest currently-active layer on
    /// (or unlocks it if already locked), so a momentary / one-shot / tap-toggle
    /// layer stays active after its key lifts.
    LayerLock,
    /// Auto-shift control key, carrying the [`AutoShiftAction`] it performs (toggle /
    /// on / off). It carries no HID usage: [`crate::keymap::compute_report`] routes a
    /// press to the [`crate::timed`] auto-shift engine, which holds the runtime
    /// enable flag.
    AutoShift(AutoShiftAction),
    /// Leader key (`LEADER`). It carries no HID usage: a press starts a leader
    /// sequence in the [`crate::timed`] engine, which captures the next keys and
    /// matches them against the host-uploaded sequence table.
    Leader,
    /// Caps-word key (`CapsWord`). It carries no HID usage: a press engages the
    /// caps-word behaviour, which holds Left Shift on word keys (letters, digits, `-`,
    /// backspace) until a non-word key ends the word. Driven on the press edge by the
    /// caps-word plugin in [`crate::features`].
    CapsWord,
    /// Key-lock key (`KeyLock`). It carries no HID usage: a press arms the key-lock
    /// behaviour so the next key pressed latches held until it is pressed again.
    /// Driven on the press edge by the key-lock plugin in [`crate::features`].
    KeyLock,
    /// Repeat key (`Repeat`). It carries no HID usage: a press re-emits the last
    /// emitted key and the modifiers held with it, through the repeat-key plugin in
    /// [`crate::features`].
    Repeat,
    /// Alternate-repeat key (`AltRepeat`). Like [`Repeat`](Self::Repeat), but re-emits the
    /// last key's alternate (e.g. Left for Right) from the plugin's mapping table.
    AltRepeat,
    /// One-shot-modifier key (`OSM`), carrying the HID modifier bit index (`0..=7`, the
    /// same bit it sets in the report modifier byte). It carries no HID usage: a press
    /// arms the one-shot-modifier plugin in [`crate::features`] so the modifier applies
    /// to the next key, then auto-releases.
    OneShotMod(u8),
    /// Default-layer key (`DF(n)`), carrying the target base layer index. It carries
    /// no HID usage: a press makes layer `n` the persistent base the active mask
    /// starts from ([`crate::keymap::compute_report`]), which persists across scans
    /// and (via the config blob) across reboots.
    DefaultLayer(u8),
    /// Grave-escape key (`GraveEscape`). It emits Escape normally, but the grave usage
    /// (`` ` ``, which Shift turns into `~`) while any Shift or GUI modifier is held —
    /// [`crate::keymap::compute_report`] picks the usage from the live modifier byte.
    GraveEscape,
    /// Space-Cadet shift key, carrying the [`SpaceCadet`] role it performs. It carries
    /// no direct HID usage: like [`ModTap`](Self::ModTap) it rides the [`crate::timed`]
    /// tap-hold engine, which emits the paren / Enter usage on a tap and asserts the
    /// modifier on a hold.
    SpaceCadet(SpaceCadet),
    /// Autocorrect control key (`AUTOCORRECT_TOGGLE` / `AUTOCORRECT_ON` / `AUTOCORRECT_OFF`), carrying the
    /// [`AutocorrectAction`] it performs. It carries no HID usage: a press drives the
    /// autocorrect plugin in [`crate::features`] on its press edge.
    Autocorrect(AutocorrectAction),
    /// Unicode mode-cycle key (`UNICODE_MODE_CYCLE`). It carries no HID usage: a press
    /// advances the active OS input mode of the [`crate::features::unicode`] sender.
    UnicodeCycle,
    /// Unicode-map key (`UM(n)`), carrying the codepoint slot index (`0..=15`). It carries
    /// no HID usage: a press makes the unicode plugin emit that slot's codepoint as the
    /// active OS mode's key sequence, injected over subsequent scans like a macro.
    UnicodeMap(u8),
}

/// The runtime control a [`KeyAction::AutoShift`] key performs on the auto-shift
/// engine's enable flag ([`crate::timed`]).
///
/// The discriminants (`1..=3`) are the offsets into the behaviour-control region
/// (`0x5A01..=0x5A03`) that [`Keycode::classify`] decodes and [`Keycode::auto_shift`]
/// encodes, so the two round-trip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AutoShiftAction {
    /// Flip the auto-shift enable flag.
    Toggle,
    /// Turn auto-shift on.
    On,
    /// Turn auto-shift off.
    Off,
}

/// The runtime control an [`KeyAction::Autocorrect`] key performs on the autocorrect
/// plugin's enable flag ([`crate::features`]).
///
/// The discriminants (`0..=2`) are the offsets into the autocorrect control region
/// (`0x5A40..=0x5A42`) that [`Keycode::classify`] decodes and [`Keycode::autocorrect`]
/// encodes, so the two round-trip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum AutocorrectAction {
    /// Flip the autocorrect enable flag.
    Toggle,
    /// Turn autocorrect on.
    On,
    /// Turn autocorrect off.
    Off,
}

/// The role a [`KeyAction::SpaceCadet`] key performs: a shift (or shift-like) key that
/// emits a symbol on a tap and asserts its modifier on a hold, resolved by the
/// [`crate::timed`] tap-hold engine.
///
/// The discriminants (`0..=2`) are the offsets into the Space-Cadet region
/// (`0x5A31..=0x5A33`) that [`Keycode::classify`] decodes and [`Keycode::space_cadet`]
/// encodes, so the two round-trip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpaceCadet {
    /// Left Shift held; tap emits `(` (Left Shift + `9`). QMK's `SC_LSPO`.
    LeftShiftParen,
    /// Right Shift held; tap emits `)` (Right Shift + `0`). QMK's `SC_RSPC`.
    RightShiftParen,
    /// Right Shift held; tap emits Enter. QMK's `SC_SENT`.
    RightShiftEnter,
}

impl SpaceCadet {
    /// The HID facts this role resolves to as `(hold_mods, tap_usage, tap_mods)`: the
    /// modifier byte a hold asserts, the basic usage a tap emits, and the modifier
    /// byte that rides that tap (so a tap of a paren key sends the shifted symbol).
    /// The [`crate::timed`] engine reads these to drive the tap-hold decision.
    pub const fn resolve(self) -> (u8, u8, u8) {
        // Modifier bits: Left Shift = bit 1 (`0x02`), Right Shift = bit 5 (`0x20`).
        // Usages: `9` = 0x26, `0` = 0x27, Enter = 0x28.
        match self {
            SpaceCadet::LeftShiftParen => (1 << 1, 0x26, 1 << 1),
            SpaceCadet::RightShiftParen => (1 << 5, 0x27, 1 << 5),
            SpaceCadet::RightShiftEnter => (1 << 5, 0x28, 0),
        }
    }
}

/// The nine USB HID mouse actions a [`KeyAction::Mouse`] can name: four-way
/// pointer movement, the three standard buttons, and wheel up/down.
///
/// The discriminants (`0..=8`) are the low-byte offsets into the mouse keycode
/// region (`0x5500..=0x5508`) that [`Keycode::classify`] decodes and
/// [`Keycode::mouse_key`] encodes, so the two round-trip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MouseKey {
    /// Move the pointer up (−Y).
    Up,
    /// Move the pointer down (+Y).
    Down,
    /// Move the pointer left (−X).
    Left,
    /// Move the pointer right (+X).
    Right,
    /// Button 1 (left click).
    Btn1,
    /// Button 2 (right click).
    Btn2,
    /// Button 3 (middle click).
    Btn3,
    /// Wheel up (scroll away from the user).
    WheelUp,
    /// Wheel down (scroll toward the user).
    WheelDown,
}

/// The gamepad actions a [`KeyAction::Gamepad`] can name: sixteen momentary buttons
/// and the eight signed ends of the four DirectInput-style axes (left stick X/Y,
/// right stick Z/Rz).
///
/// The region offset (`0x5600..=0x5617`) that [`Keycode::classify`] decodes and
/// [`Keycode::gamepad_key`] encodes is `0..=15` for [`Button`](Self::Button)`(n)`
/// and `16..=23` for the axis variants in declaration order, so the two round-trip.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GamepadKey {
    /// Button `n` (`0..=15`), reported as HID Button usage `n + 1`. Momentary: the
    /// bit is set only while the key is held.
    Button(u8),
    /// Drive the left-stick X axis to its negative end (−127, e.g. left).
    AxisXNeg,
    /// Drive the left-stick X axis to its positive end (+127, e.g. right).
    AxisXPos,
    /// Drive the left-stick Y axis negative (−127, e.g. up).
    AxisYNeg,
    /// Drive the left-stick Y axis positive (+127, e.g. down).
    AxisYPos,
    /// Drive the right-stick Z axis negative (−127).
    AxisZNeg,
    /// Drive the right-stick Z axis positive (+127).
    AxisZPos,
    /// Drive the right-stick Rz axis negative (−127).
    AxisRzNeg,
    /// Drive the right-stick Rz axis positive (+127).
    AxisRzPos,
}

/// Base of the momentary-layer (`MO`) encoding region.
const MO_BASE: u16 = 0x5200;
/// Mask selecting the layer index out of an `MO` keycode (layers `0..=31`). The
/// `TO`/`TG`/`TT`/`OSL` regions reuse it, since they encode a layer the same way.
const MO_LAYER_MASK: u16 = 0x001F;
/// Base of the activate-layer (`TO`) region: `0x5300..=0x531F`, layer in the low
/// five bits ([`MO_LAYER_MASK`]). Shares the `MO` layout; named for QMK's `TO`
/// mnemonic, but the numeric base is the firmware's own.
const TO_BASE: u16 = 0x5300;
/// Base of the toggle-layer (`TG`) region: `0x5400..=0x541F`. Shares the `MO`
/// layout; named for QMK's `TG` mnemonic, firmware-chosen base.
const TG_BASE: u16 = 0x5400;
/// Base of the tap-toggle-layer (`TT`) region: `0x5800..=0x581F`. Shares the `MO`
/// layout; named for QMK's `TT` mnemonic, firmware-chosen base.
const TT_BASE: u16 = 0x5800;
/// Base of the one-shot-layer (`OSL`) region: `0x5900..=0x591F`. Shares the `MO`
/// layout; named for QMK's `OSL` mnemonic, firmware-chosen base.
const OSL_BASE: u16 = 0x5900;
/// Base of the mod-tap (`MT`) region: `0x2000..=0x3FFF`. The low byte holds the tap
/// usage; bits `8..=12` hold a 5-bit QMK-style modifier selector. Mirrors QMK's
/// `QK_MOD_TAP`.
const MOD_TAP_BASE: u16 = 0x2000;
/// Base of the layer-tap (`LT`) region: `0x4000..=0x4FFF`. The low byte holds the tap
/// usage; bits `8..=11` hold a 4-bit layer index. Mirrors QMK's `QK_LAYER_TAP`.
const LAYER_TAP_BASE: u16 = 0x4000;
/// Mask selecting the tap usage out of an `MT`/`LT` keycode (the low byte).
const TAP_HOLD_KC_MASK: u16 = 0x00FF;
/// Bit offset of the modifier/layer selector within an `MT`/`LT` keycode.
const TAP_HOLD_SEL_SHIFT: u16 = 8;
/// Mask selecting the 5-bit modifier selector out of an `MT` keycode (after the
/// shift): bit 4 = right side, bits `0..=3` = Ctrl/Shift/Alt/GUI.
const MOD_TAP_SEL_MASK: u16 = 0x1F;
/// Mask selecting the 4-bit layer out of an `LT` keycode (after the shift), layers
/// `0..=15`.
const LAYER_TAP_LAYER_MASK: u16 = 0x0F;
/// First HID usage that denotes a modifier (`LCtrl`); the eight run to `0xE7`.
const MOD_USAGE_LO: u16 = 0x00E0;
/// Last HID usage that denotes a modifier (`RGui`).
const MOD_USAGE_HI: u16 = 0x00E7;
/// Base of the consumer-control encoding region: keycodes `0xC000..=0xFFFF`
/// carry an HID consumer usage (usage page `0x0C`). The top quarter of the
/// `u16` space is reserved for it so it cannot overlap `NO`/`TRNS`/basic/
/// modifier (`< 0x0100`) or `MO` (`0x52xx`).
const CONSUMER_BASE: u16 = 0xC000;
/// Mask selecting the consumer usage out of a consumer keycode. The region
/// spans `0x4000` codes, so the low 14 bits comfortably hold the whole consumer
/// usage page.
const CONSUMER_USAGE_MASK: u16 = 0x3FFF;
/// Base of the tap-dance (`TD`) encoding region: `0x5700..=0x57FF` carry an
/// 8-bit tap-dance entry index in the low byte. Mirrors QMK's `QK_TAP_DANCE`.
const TAP_DANCE_BASE: u16 = 0x5700;
/// Base of the dynamic-macro (`MACRO`) encoding region: `0x7700..=0x77FF` carry
/// an 8-bit macro entry index in the low byte. Mirrors QMK's `QK_MACRO`.
const MACRO_BASE: u16 = 0x7700;
/// Mask selecting the entry index out of a `TD`/`MACRO` keycode (`0..=255`).
const ENTRY_INDEX_MASK: u16 = 0x00FF;
/// Base of the mouse-key region: `0x5500..=0x55FF` is reserved for USB HID mouse
/// actions, of which `0x5500..=0x5508` are currently assigned (see [`MouseKey`]).
/// Sits in the gap between `MO` (`<= 0x521F`) and `TD` (`>= 0x5700`).
const MOUSE_BASE: u16 = 0x5500;
/// Number of assigned mouse keys — the [`MouseKey`] variants (`0..=8`).
const MOUSE_KEY_COUNT: u16 = 9;
/// Base of the gamepad-key region: `0x5600..=0x56FF` is reserved for the gamepad
/// actions, of which `0x5600..=0x5617` are currently assigned (sixteen buttons then
/// eight axis-direction keys; see [`GamepadKey`]). Sits in the gap between mouse
/// (`<= 0x55FF`) and `TD` (`>= 0x5700`).
const GAMEPAD_BASE: u16 = 0x5600;
/// Number of gamepad buttons (region offsets `0..=15`), reported as HID Button
/// usages `1..=16`.
const GAMEPAD_BUTTON_COUNT: u16 = 16;
/// Number of gamepad axis-direction keys (region offsets `16..=23`): the two signed
/// ends of each of the four axes (left stick X/Y, right stick Z/Rz).
const GAMEPAD_AXIS_COUNT: u16 = 8;
/// Number of assigned gamepad keys — buttons plus axis-direction keys.
const GAMEPAD_KEY_COUNT: u16 = GAMEPAD_BUTTON_COUNT + GAMEPAD_AXIS_COUNT;
/// The bootloader-entry keycode, `BOOTLOADER` (`0x7C00`). Mirrors QMK's
/// `QK_BOOTLOADER`/`QK_BOOT`; it is a single internal control code with no HID
/// usage, sitting in the gap between `MACRO` (`<= 0x77FF`) and consumer
/// (`>= 0xC000`).
const BOOT_KEYCODE: u16 = 0x7C00;
/// Base of the firmware behaviour-control region: `0x5A00..=0x5A08` holds the
/// parameterless control codes the keymap / timed engines and the [`crate::features`]
/// plugins act on directly — layer lock (`0x5A00`), the three auto-shift toggles
/// (`0x5A01..=0x5A03`), the leader trigger (`0x5A04`), caps-word (`0x5A05`), key-lock
/// (`0x5A06`) and the two repeat keys (`0x5A07`/`0x5A08`). Sits in the gap between
/// `OSL` (`<= 0x591F`) and `MACRO` (`>= 0x7700`).
const BEHAVIOR_KC_BASE: u16 = 0x5A00;
/// Number of assigned parameterless behaviour-control codes (`0x5A00..=0x5A08`): the
/// layer-lock, three auto-shift, leader, caps-word, key-lock and two repeat codes.
const BEHAVIOR_KC_COUNT: u16 = 9;
/// Base of the one-shot-modifier (`OSM`) region: `0x5A09..=0x5A10` carry the HID
/// modifier bit index (`0..=7`) as the offset from this base, so one code covers each
/// of the eight modifiers (`LCtrl..RGui`). Sits directly above the parameterless
/// behaviour codes and inside the same behaviour region.
const OSM_BASE: u16 = 0x5A09;
/// Number of one-shot-modifier codes — one per HID modifier bit (`0..=7`).
const OSM_MOD_COUNT: u16 = 8;

// The behaviour-control region (`0x5A00..=0x5AFF`) is partitioned into disjoint
// sub-blocks: `0x5A00..=0x5A04` are the layer-lock / auto-shift / leader codes,
// `0x5A05..=0x5A10` the caps-word / key-lock / repeat / one-shot-mod codes
// (`0x5A11..=0x5A1F` reserved), `0x5A20..=0x5A3F` the default-layer / grave-escape /
// space-cadet block, and `0x5A40..=0x5A4F` the autocorrect block (below).
/// Base of the default-layer (`DF`) region (`0x5A20..=`): the offset from the base is
/// the target base layer index. Mirrors QMK's `QK_DEF_LAYER` intent (the firmware
/// never relies on QMK's numbering).
const DEFAULT_LAYER_BASE: u16 = 0x5A20;
/// Number of assigned default-layer codes — one per keymap layer.
const DEFAULT_LAYER_COUNT: u16 = crate::keymap::LAYERS as u16;
/// The grave-escape (`GraveEscape`) control code (`0x5A30`).
const GESC_KEYCODE: u16 = 0x5A30;
/// Base of the Space-Cadet region (`0x5A31..=0x5A33`): the offset from the base is the
/// [`SpaceCadet`] role.
const SPACE_CADET_BASE: u16 = 0x5A31;
/// Number of assigned Space-Cadet codes — the three [`SpaceCadet`] roles.
const SPACE_CADET_COUNT: u16 = 3;
/// Inclusive base/top of the default-layer / grave-escape / space-cadet sub-block
/// (`0x5A20..=0x5A3F`).
const DEFAULT_LAYER_BLOCK_BASE: u16 = 0x5A20;
const DEFAULT_LAYER_BLOCK_TOP: u16 = 0x5A3F;
/// Base of the autocorrect control region (`0x5A40..=0x5A42`): the offset from the base is
/// the [`AutocorrectAction`] (toggle / on / off).
const AUTOCORRECT_KC_BASE: u16 = 0x5A40;
/// Number of assigned autocorrect control codes — the three [`AutocorrectAction`]s.
const AUTOCORRECT_KC_COUNT: u16 = 3;
/// Inclusive base/top of the autocorrect behaviour-control sub-block (`0x5A40..=0x5A4F`).
const AUTOCORRECT_BLOCK_BASE: u16 = 0x5A40;
const AUTOCORRECT_BLOCK_TOP: u16 = 0x5A4F;

// The Unicode-input sub-block (`0x5A50..=0x5A60`) is the next slice of the behaviour
// region, sitting clear of every sub-block below it (`0x5A00..=0x5A4F`) so a Unicode
// code can never collide with one of those.
/// The Unicode mode-cycle control code (`0x5A50`): a press cycles the active OS input
/// mode the [`crate::features::unicode`] sender targets.
const UNICODE_CYCLE_KEYCODE: u16 = 0x5A50;
/// Base of the Unicode-map (`UM`) region (`0x5A51..=0x5A60`): the offset from the base
/// is the codepoint slot the sender emits.
const UNICODE_MAP_BASE: u16 = 0x5A51;
/// Number of Unicode-map slots — one `UM(n)` code per host-uploaded codepoint.
const UNICODE_MAP_COUNT: u16 = 16;
/// Inclusive top of the assigned `UM` codes (`UNICODE_MAP_BASE + 15 = 0x5A60`).
const UNICODE_MAP_TOP: u16 = UNICODE_MAP_BASE + UNICODE_MAP_COUNT - 1;
/// Inclusive base/top of the Unicode sub-block (`0x5A50..=0x5A60`).
const UNICODE_BLOCK_BASE: u16 = 0x5A50;
const UNICODE_BLOCK_TOP: u16 = 0x5A60;

/// Inclusive top of the `MO` region: `MO_BASE` plus the full layer mask
/// (`0x521F`). Single source for the [`Keycode::classify`] range pattern and the
/// disjointness assertions below.
const MO_TOP: u16 = MO_BASE + MO_LAYER_MASK;
/// Inclusive tops of the `TO`/`TG`/`TT`/`OSL` regions: each base plus the shared
/// layer mask (`…1F`). Single source for the [`Keycode::classify`] ranges and the
/// disjointness assertions below.
const TO_TOP: u16 = TO_BASE + MO_LAYER_MASK;
const TG_TOP: u16 = TG_BASE + MO_LAYER_MASK;
const TT_TOP: u16 = TT_BASE + MO_LAYER_MASK;
const OSL_TOP: u16 = OSL_BASE + MO_LAYER_MASK;
/// Inclusive top of the `MT` region: base plus the full selector and tap fields
/// (`0x3FFF`).
const MOD_TAP_TOP: u16 = MOD_TAP_BASE | (MOD_TAP_SEL_MASK << TAP_HOLD_SEL_SHIFT) | TAP_HOLD_KC_MASK;
/// Inclusive top of the `LT` region: base plus the full layer and tap fields
/// (`0x4FFF`).
const LAYER_TAP_TOP: u16 =
    LAYER_TAP_BASE | (LAYER_TAP_LAYER_MASK << TAP_HOLD_SEL_SHIFT) | TAP_HOLD_KC_MASK;
/// Inclusive top of the `TD` region: `TAP_DANCE_BASE` plus the full entry mask
/// (`0x57FF`).
const TD_TOP: u16 = TAP_DANCE_BASE + ENTRY_INDEX_MASK;
/// Inclusive top of the `MACRO` region: `MACRO_BASE` plus the full entry mask
/// (`0x77FF`).
const MACRO_TOP: u16 = MACRO_BASE + ENTRY_INDEX_MASK;
/// Inclusive top of the *assigned* mouse codes (`MOUSE_BASE + 8 = 0x5508`); codes
/// `0x5509..=0x55FF` are reserved-but-unassigned and [`Keycode::classify`] them as
/// [`KeyAction::NoOp`].
const MOUSE_VALID_TOP: u16 = MOUSE_BASE + MOUSE_KEY_COUNT - 1;
/// Inclusive top of the reserved mouse region (`0x55FF`), for the disjointness
/// assertions below.
const MOUSE_REGION_TOP: u16 = MOUSE_BASE + 0x00FF;
/// Inclusive top of the *assigned* gamepad codes (`GAMEPAD_BASE + 23 = 0x5617`);
/// codes `0x5618..=0x56FF` are reserved-but-unassigned and [`Keycode::classify`]
/// them as [`KeyAction::NoOp`].
const GAMEPAD_VALID_TOP: u16 = GAMEPAD_BASE + GAMEPAD_KEY_COUNT - 1;
/// Inclusive top of the reserved gamepad region (`0x56FF`), for the disjointness
/// assertions below.
const GAMEPAD_REGION_TOP: u16 = GAMEPAD_BASE + 0x00FF;
/// Inclusive top of the *assigned* parameterless behaviour-control codes
/// (`BEHAVIOR_KC_BASE + 8 = 0x5A08`). The `OSM` codes follow at `0x5A09..=0x5A10`; the
/// rest of the region (`0x5A11..=0x5AFF`) is reserved-but-unassigned and
/// [`Keycode::classify`] decodes it as [`KeyAction::NoOp`].
const BEHAVIOR_KC_VALID_TOP: u16 = BEHAVIOR_KC_BASE + BEHAVIOR_KC_COUNT - 1;
/// Inclusive top of the *assigned* `OSM` codes (`OSM_BASE + 7 = 0x5A10`).
const OSM_VALID_TOP: u16 = OSM_BASE + OSM_MOD_COUNT - 1;
/// Inclusive top of the reserved behaviour-control region (`0x5AFF`), for the
/// disjointness assertions below.
const BEHAVIOR_KC_REGION_TOP: u16 = BEHAVIOR_KC_BASE + 0x00FF;
/// Inclusive top of the assigned default-layer codes (`DEFAULT_LAYER_BASE + LAYERS - 1`).
const DEFAULT_LAYER_TOP: u16 = DEFAULT_LAYER_BASE + DEFAULT_LAYER_COUNT - 1;
/// Inclusive top of the assigned Space-Cadet codes (`0x5A33`).
const SPACE_CADET_TOP: u16 = SPACE_CADET_BASE + SPACE_CADET_COUNT - 1;
/// Inclusive top of the assigned autocorrect control codes (`0x5A42`).
const AUTOCORRECT_KC_VALID_TOP: u16 = AUTOCORRECT_KC_BASE + AUTOCORRECT_KC_COUNT - 1;

// Compile-time proof that the two new 256-code windows are disjoint from every
// existing region. `TD` sits strictly above `MO`'s top (`0x521F`) and strictly
// below `MACRO`'s base; `MACRO` sits strictly below consumer's base (`0xC000`).
// Any overlap would make a single keycode classify two ways — these assertions
// fail the build before that can happen.
const _: () = assert!(MO_TOP < TAP_DANCE_BASE, "TD must sit above MO");
const _: () = assert!(TD_TOP < MACRO_BASE, "TD must sit below MACRO");
const _: () = assert!(MACRO_TOP < CONSUMER_BASE, "MACRO must sit below consumer");
// The mouse region (`0x5500..=0x55FF`) sits strictly between `MO`'s top (`0x521F`)
// and `TD`'s base (`0x5700`), so a mouse keycode can never classify two ways.
const _: () = assert!(MO_TOP < MOUSE_BASE, "mouse region must sit above MO");
const _: () = assert!(MOUSE_REGION_TOP < TAP_DANCE_BASE, "mouse region must sit below TD");
// The gamepad region (`0x5600..=0x56FF`) sits strictly between mouse's top
// (`0x55FF`) and `TD`'s base (`0x5700`), so a gamepad keycode can never classify
// two ways.
const _: () = assert!(MOUSE_REGION_TOP < GAMEPAD_BASE, "gamepad region must sit above mouse");
const _: () = assert!(GAMEPAD_REGION_TOP < TAP_DANCE_BASE, "gamepad region must sit below TD");
const _: () = assert!(MOD_USAGE_HI < 0x0100, "modifiers must stay below 0x0100");
// The mod-tap (`0x2000..=0x3FFF`) and layer-tap (`0x4000..=0x4FFF`) regions sit in
// the unassigned span between the basic usages (`< 0x0100`) and `MO` (`0x5200`),
// strictly disjoint from those and from each other — so an `MT`/`LT` keycode can
// never classify two ways. These assertions fail the build before that can happen.
const _: () = assert!(0x00FF < MOD_TAP_BASE, "mod-tap must sit above basic usages");
const _: () = assert!(MOD_TAP_TOP < LAYER_TAP_BASE, "mod-tap must sit below layer-tap");
const _: () = assert!(LAYER_TAP_TOP < MO_BASE, "layer-tap must sit below MO");
// `BOOTLOADER` is a single code in the gap above `MACRO`'s top (`0x77FF`) and below
// consumer's base (`0xC000`), so it cannot collide with any other region.
const _: () = assert!(MACRO_TOP < BOOT_KEYCODE, "BOOTLOADER must sit above MACRO");
const _: () = assert!(BOOT_KEYCODE < CONSUMER_BASE, "BOOTLOADER must sit below consumer");

// The four added layer-switch windows are disjoint from MO, TD, MACRO and each
// other: `TO`/`TG` slot between MO's top (`0x521F`) and TD's base, and `TT`/`OSL`
// between TD's top (`0x57FF`) and MACRO's base. Any overlap would classify a code
// two ways — these assertions fail the build first.
const _: () = assert!(MO_TOP < TO_BASE, "TO must sit above MO");
const _: () = assert!(TO_TOP < TG_BASE, "TG must sit above TO");
const _: () = assert!(TG_TOP < TAP_DANCE_BASE, "TG must sit below TD");
const _: () = assert!(TD_TOP < TT_BASE, "TT must sit above TD");
const _: () = assert!(TT_TOP < OSL_BASE, "OSL must sit above TT");
const _: () = assert!(OSL_TOP < MACRO_BASE, "OSL must sit below MACRO");

// The behaviour-control region (`0x5A00..=0x5AFF`) slots between OSL's top
// (`0x591F`) and MACRO's base (`0x7700`), so a behaviour-control keycode can never
// classify two ways — these assertions fail the build before that can happen.
const _: () = assert!(OSL_TOP < BEHAVIOR_KC_BASE, "behaviour codes must sit above OSL");
const _: () = assert!(
    BEHAVIOR_KC_REGION_TOP < MACRO_BASE,
    "behaviour codes must sit below MACRO"
);
// `OSM` (`0x5A09..=0x5A10`) sits directly above the parameterless behaviour codes and
// inside the same behaviour region, so it cannot classify two ways.
const _: () = assert!(
    BEHAVIOR_KC_VALID_TOP < OSM_BASE,
    "OSM must sit above the parameterless behaviour codes"
);
const _: () = assert!(
    OSM_VALID_TOP <= BEHAVIOR_KC_REGION_TOP,
    "OSM must stay inside the behaviour region"
);

// The default-layer / grave-escape / space-cadet block (`0x5A20..=0x5A3F`) sits within
// the behaviour-control region, strictly above the parameterless behaviour codes and the
// caps-word / key-lock / repeat / one-shot-mod codes and reserved tail below it
// (`<= 0x5A1F`), so a code here can never classify two ways nor collide with a lower
// sub-block. Its three sub-regions are internally ordered and disjoint: default-layer,
// then grave-escape, then space-cadet.
const _: () = assert!(
    BEHAVIOR_KC_VALID_TOP < DEFAULT_LAYER_BLOCK_BASE,
    "default-layer block must sit above the parameterless behaviour codes"
);
const _: () = assert!(
    0x5A1F < DEFAULT_LAYER_BLOCK_BASE,
    "default-layer block must sit above the caps-word / key-lock / repeat / one-shot-mod codes and reserved tail (0x5A05..=0x5A1F)"
);
const _: () = assert!(
    DEFAULT_LAYER_BLOCK_TOP <= BEHAVIOR_KC_REGION_TOP,
    "default-layer block must stay within the behaviour-control region"
);
const _: () = assert!(
    DEFAULT_LAYER_BASE == DEFAULT_LAYER_BLOCK_BASE && DEFAULT_LAYER_TOP < GESC_KEYCODE,
    "default-layer codes lead the block, below GraveEscape"
);
const _: () = assert!(GESC_KEYCODE < SPACE_CADET_BASE, "GraveEscape must sit below Space-Cadet");
const _: () = assert!(
    SPACE_CADET_TOP <= DEFAULT_LAYER_BLOCK_TOP,
    "Space-Cadet codes must stay within the default-layer block"
);

// The autocorrect sub-block (`0x5A40..=0x5A4F`) sits within the behaviour-control region,
// strictly above the default-layer / grave-escape / space-cadet block (`<= 0x5A3F`), so an
// autocorrect code can never classify two ways nor collide with a lower sub-block.
const _: () = assert!(
    DEFAULT_LAYER_BLOCK_TOP < AUTOCORRECT_BLOCK_BASE,
    "autocorrect block must sit above the default-layer / grave-escape / space-cadet block"
);
const _: () = assert!(
    AUTOCORRECT_BLOCK_TOP <= BEHAVIOR_KC_REGION_TOP,
    "autocorrect block must stay within the behaviour-control region"
);
const _: () = assert!(
    AUTOCORRECT_KC_BASE == AUTOCORRECT_BLOCK_BASE
        && AUTOCORRECT_KC_VALID_TOP <= AUTOCORRECT_BLOCK_TOP,
    "autocorrect codes must stay within the autocorrect block"
);

// The Unicode sub-block (`0x5A50..=0x5A60`) sits within the behaviour-control region,
// strictly above the default-layer block (`<= 0x5A3F`) and the autocorrect `0x5A40..=0x5A42`
// codes, so a Unicode code can never classify two ways nor collide with a lower sub-block.
// The mode-cycle code leads, then the contiguous `UM` map fills the rest of the block.
const _: () = assert!(
    DEFAULT_LAYER_BLOCK_TOP < UNICODE_BLOCK_BASE,
    "Unicode block must sit above the default-layer block"
);
const _: () = assert!(
    0x5A42 < UNICODE_BLOCK_BASE,
    "Unicode block must sit above the autocorrect 0x5A40..=0x5A42 codes"
);
const _: () = assert!(
    UNICODE_BLOCK_BASE == UNICODE_CYCLE_KEYCODE && UNICODE_CYCLE_KEYCODE < UNICODE_MAP_BASE,
    "the mode-cycle code leads the Unicode block, below the UM map"
);
const _: () = assert!(
    UNICODE_MAP_TOP == UNICODE_BLOCK_TOP,
    "the UM map fills the Unicode block to its top"
);
const _: () = assert!(
    UNICODE_BLOCK_TOP <= BEHAVIOR_KC_REGION_TOP,
    "Unicode block must stay within the behaviour-control region"
);

/// Expand an `MT` 5-bit modifier selector into the HID modifier byte. The low
/// nibble selects Ctrl/Shift/Alt/GUI; bit 4 picks the right-hand side, shifting the
/// nibble into the byte's high half (`RCtrl..RGui`) — the inverse of
/// [`compress_mods`], so the two round-trip on every single-sided selector.
const fn expand_mods(sel: u8) -> u8 {
    let nibble = sel & 0x0F;
    if sel & 0x10 != 0 {
        nibble << 4
    } else {
        nibble
    }
}

/// Compress a single-sided HID modifier byte into the `MT` 5-bit selector. A byte
/// with any right-hand modifier (`0xF0`) is encoded right-side (bit 4 set, high
/// nibble selecting the modifiers); otherwise it is left-side. A mixed-side byte is
/// not representable and resolves right-side — the documented bound on
/// [`Keycode::mod_tap`].
const fn compress_mods(mods: u8) -> u8 {
    if mods & 0xF0 != 0 {
        0x10 | (mods >> 4)
    } else {
        mods & 0x0F
    }
}

impl Keycode {
    /// No-op: an unbound key.
    pub const NO: Keycode = Keycode(0x0000);
    /// Transparent: fall through to the next active layer below.
    pub const TRNS: Keycode = Keycode(0x0001);

    /// Build a keycode from a raw basic HID keyboard usage (usage page `0x07`).
    ///
    /// Usages `0xE0..=0xE7` construct the corresponding modifier; every other
    /// in-range usage is a basic key.
    pub const fn from_usage(usage: u8) -> Keycode {
        Keycode(usage as u16)
    }

    /// Build a momentary layer switch, `MO(layer)`.
    ///
    /// Active only while the key is held. `layer` must be `0..=31`; higher bits
    /// are masked off.
    pub const fn momentary(layer: u8) -> Keycode {
        Keycode(MO_BASE | (layer as u16 & MO_LAYER_MASK))
    }

    /// Build an activate-layer switch, `TO(layer)`: on press it makes `layer` the
    /// only active non-base layer. `layer` must be `0..=31`; higher bits are masked.
    pub const fn to_layer(layer: u8) -> Keycode {
        Keycode(TO_BASE | (layer as u16 & MO_LAYER_MASK))
    }

    /// Build a toggle-layer switch, `TG(layer)`: each press flips whether `layer`
    /// is latched active. `layer` must be `0..=31`; higher bits are masked.
    pub const fn toggle(layer: u8) -> Keycode {
        Keycode(TG_BASE | (layer as u16 & MO_LAYER_MASK))
    }

    /// Build a tap-or-hold layer switch, `TT(layer)`: momentary while held, but a
    /// bare tap toggles `layer`. `layer` must be `0..=31`; higher bits are masked.
    pub const fn tap_toggle(layer: u8) -> Keycode {
        Keycode(TT_BASE | (layer as u16 & MO_LAYER_MASK))
    }

    /// Build a one-shot layer switch, `OSL(layer)`: `layer` stays active until the
    /// next key press. `layer` must be `0..=31`; higher bits are masked.
    pub const fn one_shot(layer: u8) -> Keycode {
        Keycode(OSL_BASE | (layer as u16 & MO_LAYER_MASK))
    }

    /// Build a mod-tap, `MT(mods, kc)`: tap emits the basic usage `kc`, hold asserts
    /// `mods` (the HID modifier byte). The byte is compressed into the 5-bit QMK-style
    /// selector ([`compress_mods`]), so it must be **single-sided** — only the left
    /// (`0x0F`) *or* only the right (`0xF0`) modifiers — which is the encoding's bound
    /// (a one-sided Ctrl/Shift/Alt/GUI combination is fine).
    pub const fn mod_tap(mods: u8, kc: u8) -> Keycode {
        Keycode(MOD_TAP_BASE | ((compress_mods(mods) as u16) << TAP_HOLD_SEL_SHIFT) | kc as u16)
    }

    /// Build a layer-tap, `LT(layer, kc)`: tap emits the basic usage `kc`, hold
    /// momentarily activates `layer`. `layer` must be `0..=15`; higher bits are masked.
    pub const fn layer_tap(layer: u8, kc: u8) -> Keycode {
        Keycode(
            LAYER_TAP_BASE
                | ((layer as u16 & LAYER_TAP_LAYER_MASK) << TAP_HOLD_SEL_SHIFT)
                | kc as u16,
        )
    }

    /// Build a consumer-control keycode from a raw HID consumer usage (usage
    /// page `0x0C`), e.g. `0xE9` (Volume Increment) or `0xCD` (Play/Pause).
    ///
    /// The usage is held in the low 14 bits; values above `0x3FFF` (none are
    /// defined on the consumer page) are masked off.
    pub const fn consumer(usage: u16) -> Keycode {
        Keycode(CONSUMER_BASE | (usage & CONSUMER_USAGE_MASK))
    }

    /// Build a tap-dance keycode, `TD(index)`, naming entry `index` of the timed
    /// engine's tap-dance table. The index is held in the low byte.
    pub const fn tap_dance(index: u8) -> Keycode {
        Keycode(TAP_DANCE_BASE | index as u16)
    }

    /// Build a dynamic-macro keycode, `MACRO(index)`, naming entry `index` of the
    /// timed engine's macro table. The index is held in the low byte.
    pub const fn macro_entry(index: u8) -> Keycode {
        Keycode(MACRO_BASE | index as u16)
    }

    /// Build a mouse keycode naming a [`MouseKey`] action (`0x5500..=0x5508`). The
    /// inverse of the [`MouseKey`] arm of [`classify`](Keycode::classify): the
    /// variant's discriminant is the region offset.
    pub const fn mouse_key(k: MouseKey) -> Keycode {
        Keycode(MOUSE_BASE | k as u16)
    }

    /// Build a gamepad keycode naming a [`GamepadKey`] action (`0x5600..=0x5617`).
    /// The inverse of the [`GamepadKey`] arm of [`classify`](Keycode::classify): a
    /// button's index `0..=15` is the region offset, and the axis variants take the
    /// fixed offsets `16..=23` in declaration order.
    pub const fn gamepad_key(k: GamepadKey) -> Keycode {
        let off = match k {
            // Mask to the sixteen-button sub-range so an out-of-range index can
            // never spill into the axis codes that sit just above it.
            GamepadKey::Button(n) => (n & 0x0F) as u16,
            GamepadKey::AxisXNeg => 16,
            GamepadKey::AxisXPos => 17,
            GamepadKey::AxisYNeg => 18,
            GamepadKey::AxisYPos => 19,
            GamepadKey::AxisZNeg => 20,
            GamepadKey::AxisZPos => 21,
            GamepadKey::AxisRzNeg => 22,
            GamepadKey::AxisRzPos => 23,
        };
        Keycode(GAMEPAD_BASE | off)
    }

    /// Build the layer-lock keycode, `LAYER_LOCK` (`0x5A00`). On press the keymap engine
    /// locks the highest currently-active layer on, or unlocks it if already locked.
    pub const fn layer_lock() -> Keycode {
        Keycode(BEHAVIOR_KC_BASE)
    }

    /// Build an auto-shift control keycode (`0x5A01..=0x5A03`) naming an
    /// [`AutoShiftAction`]. The inverse of the [`AutoShiftAction`] arm of
    /// [`classify`](Keycode::classify): the action's offset into the region.
    pub const fn auto_shift(action: AutoShiftAction) -> Keycode {
        let off = match action {
            AutoShiftAction::Toggle => 1,
            AutoShiftAction::On => 2,
            AutoShiftAction::Off => 3,
        };
        Keycode(BEHAVIOR_KC_BASE + off)
    }

    /// Build the leader keycode, `LEADER` (`0x5A04`). A press starts a leader
    /// sequence in the [`crate::timed`] engine.
    pub const fn leader() -> Keycode {
        Keycode(BEHAVIOR_KC_BASE + 4)
    }

    /// Build the caps-word keycode, `CapsWord` (`0x5A05`). A press engages the
    /// caps-word behaviour in [`crate::features`].
    pub const fn caps_word() -> Keycode {
        Keycode(BEHAVIOR_KC_BASE + 5)
    }

    /// Build the key-lock keycode, `KeyLock` (`0x5A06`). A press arms the key-lock
    /// behaviour so the next key latches held.
    pub const fn key_lock() -> Keycode {
        Keycode(BEHAVIOR_KC_BASE + 6)
    }

    /// Build the repeat keycode, `Repeat` (`0x5A07`). A press re-emits the last
    /// emitted key and its modifiers.
    pub const fn repeat() -> Keycode {
        Keycode(BEHAVIOR_KC_BASE + 7)
    }

    /// Build the alternate-repeat keycode, `AltRepeat` (`0x5A08`). A press re-emits the
    /// last key's alternate from the repeat plugin's mapping table.
    pub const fn alt_repeat() -> Keycode {
        Keycode(BEHAVIOR_KC_BASE + 8)
    }

    /// Build a one-shot-modifier keycode, `OSM(bit)` (`0x5A09..=0x5A10`), naming the HID
    /// modifier by its bit index (`0..=7`); higher bits are masked into range.
    pub const fn one_shot_mod(bit: u8) -> Keycode {
        Keycode(OSM_BASE + (bit as u16 & 0x07))
    }

    /// Build a default-layer keycode, `DF(layer)` (`0x5A20..`). A press makes `layer`
    /// the persistent base the active mask starts from. `layer` is masked into the
    /// assigned region; the keymap ignores a layer `>= LAYERS`.
    pub const fn default_layer(layer: u8) -> Keycode {
        Keycode(DEFAULT_LAYER_BASE + (layer as u16 % DEFAULT_LAYER_COUNT))
    }

    /// Build the grave-escape keycode, `GraveEscape` (`0x5A30`).
    pub const fn grave_escape() -> Keycode {
        Keycode(GESC_KEYCODE)
    }

    /// Build a Space-Cadet keycode (`0x5A31..=0x5A33`) naming a [`SpaceCadet`] role.
    /// The inverse of the [`SpaceCadet`] arm of [`classify`](Keycode::classify): the
    /// role's offset into the region.
    pub const fn space_cadet(role: SpaceCadet) -> Keycode {
        let off = match role {
            SpaceCadet::LeftShiftParen => 0,
            SpaceCadet::RightShiftParen => 1,
            SpaceCadet::RightShiftEnter => 2,
        };
        Keycode(SPACE_CADET_BASE + off)
    }

    /// Build an autocorrect control keycode (`0x5A40..=0x5A42`) naming an
    /// [`AutocorrectAction`]. The inverse of the [`AutocorrectAction`] arm of
    /// [`classify`](Keycode::classify): the action's offset into the region.
    pub const fn autocorrect(action: AutocorrectAction) -> Keycode {
        let off = match action {
            AutocorrectAction::Toggle => 0,
            AutocorrectAction::On => 1,
            AutocorrectAction::Off => 2,
        };
        Keycode(AUTOCORRECT_KC_BASE + off)
    }

    /// Build the Unicode mode-cycle keycode, `UNICODE_MODE_CYCLE` (`0x5A50`).
    pub const fn unicode_cycle() -> Keycode {
        Keycode(UNICODE_CYCLE_KEYCODE)
    }

    /// Build a Unicode-map keycode, `UM(slot)` (`0x5A51..=0x5A60`), naming the codepoint
    /// slot (`0..=15`) the sender emits. The inverse of the [`KeyAction::UnicodeMap`] arm
    /// of [`classify`](Keycode::classify); a slot past the map wraps into range.
    pub const fn unicode_map(slot: u8) -> Keycode {
        Keycode(UNICODE_MAP_BASE + (slot as u16 % UNICODE_MAP_COUNT))
    }

    /// Build a keycode from its raw 16-bit encoding — the inverse of [`raw`].
    ///
    /// The configuration protocol carries keycodes as opaque little-endian
    /// `u16`s and writes them straight into the keymap, so this accepts any
    /// value: an encoding outside the assigned regions simply [`classify`]es as
    /// [`KeyAction::NoOp`], which is the same safe no-op an unbound key produces.
    ///
    /// [`raw`]: Keycode::raw
    /// [`classify`]: Keycode::classify
    pub const fn from_raw(raw: u16) -> Keycode {
        Keycode(raw)
    }

    /// The raw 16-bit encoding, for diagnostics and the config protocol.
    pub const fn raw(self) -> u16 {
        self.0
    }

    /// Decode this keycode into its [`KeyAction`].
    pub const fn classify(self) -> KeyAction {
        match self.0 {
            0x0000 => KeyAction::NoOp,
            0x0001 => KeyAction::Transparent,
            MOD_USAGE_LO..=MOD_USAGE_HI => KeyAction::Modifier((self.0 - MOD_USAGE_LO) as u8),
            0x0004..=0x00DF | 0x00E8..=0x00FF => KeyAction::Key(self.0 as u8),
            MOD_TAP_BASE..=MOD_TAP_TOP => KeyAction::ModTap {
                mods: expand_mods(((self.0 >> TAP_HOLD_SEL_SHIFT) & MOD_TAP_SEL_MASK) as u8),
                kc: (self.0 & TAP_HOLD_KC_MASK) as u8,
            },
            LAYER_TAP_BASE..=LAYER_TAP_TOP => KeyAction::LayerTap {
                layer: ((self.0 >> TAP_HOLD_SEL_SHIFT) & LAYER_TAP_LAYER_MASK) as u8,
                kc: (self.0 & TAP_HOLD_KC_MASK) as u8,
            },
            MO_BASE..=MO_TOP => KeyAction::Momentary((self.0 & MO_LAYER_MASK) as u8),
            TO_BASE..=TO_TOP => KeyAction::ToLayer((self.0 & MO_LAYER_MASK) as u8),
            TG_BASE..=TG_TOP => KeyAction::Toggle((self.0 & MO_LAYER_MASK) as u8),
            MOUSE_BASE..=MOUSE_VALID_TOP => match self.0 - MOUSE_BASE {
                0 => KeyAction::Mouse(MouseKey::Up),
                1 => KeyAction::Mouse(MouseKey::Down),
                2 => KeyAction::Mouse(MouseKey::Left),
                3 => KeyAction::Mouse(MouseKey::Right),
                4 => KeyAction::Mouse(MouseKey::Btn1),
                5 => KeyAction::Mouse(MouseKey::Btn2),
                6 => KeyAction::Mouse(MouseKey::Btn3),
                7 => KeyAction::Mouse(MouseKey::WheelUp),
                // The range guard bounds the offset to `0..=8`, so `8` is the only
                // remaining case (wheel down).
                _ => KeyAction::Mouse(MouseKey::WheelDown),
            },
            GAMEPAD_BASE..=GAMEPAD_VALID_TOP => {
                let off = (self.0 - GAMEPAD_BASE) as u8;
                match off {
                    0..=15 => KeyAction::Gamepad(GamepadKey::Button(off)),
                    16 => KeyAction::Gamepad(GamepadKey::AxisXNeg),
                    17 => KeyAction::Gamepad(GamepadKey::AxisXPos),
                    18 => KeyAction::Gamepad(GamepadKey::AxisYNeg),
                    19 => KeyAction::Gamepad(GamepadKey::AxisYPos),
                    20 => KeyAction::Gamepad(GamepadKey::AxisZNeg),
                    21 => KeyAction::Gamepad(GamepadKey::AxisZPos),
                    22 => KeyAction::Gamepad(GamepadKey::AxisRzNeg),
                    // The range guard bounds the offset to `0..=23`, so `23` is the
                    // only remaining case (right-stick Rz positive).
                    _ => KeyAction::Gamepad(GamepadKey::AxisRzPos),
                }
            }
            TAP_DANCE_BASE..=TD_TOP => KeyAction::TapDance((self.0 & ENTRY_INDEX_MASK) as u8),
            TT_BASE..=TT_TOP => KeyAction::TapToggle((self.0 & MO_LAYER_MASK) as u8),
            OSL_BASE..=OSL_TOP => KeyAction::OneShot((self.0 & MO_LAYER_MASK) as u8),
            BEHAVIOR_KC_BASE..=BEHAVIOR_KC_VALID_TOP => match self.0 - BEHAVIOR_KC_BASE {
                0 => KeyAction::LayerLock,
                1 => KeyAction::AutoShift(AutoShiftAction::Toggle),
                2 => KeyAction::AutoShift(AutoShiftAction::On),
                3 => KeyAction::AutoShift(AutoShiftAction::Off),
                4 => KeyAction::Leader,
                5 => KeyAction::CapsWord,
                6 => KeyAction::KeyLock,
                7 => KeyAction::Repeat,
                // The range guard bounds the offset to `0..=8`, so `8` is the only
                // remaining case (the alternate-repeat key).
                _ => KeyAction::AltRepeat,
            },
            OSM_BASE..=OSM_VALID_TOP => KeyAction::OneShotMod((self.0 - OSM_BASE) as u8),
            DEFAULT_LAYER_BASE..=DEFAULT_LAYER_TOP => {
                KeyAction::DefaultLayer((self.0 - DEFAULT_LAYER_BASE) as u8)
            }
            GESC_KEYCODE => KeyAction::GraveEscape,
            SPACE_CADET_BASE..=SPACE_CADET_TOP => match self.0 - SPACE_CADET_BASE {
                0 => KeyAction::SpaceCadet(SpaceCadet::LeftShiftParen),
                1 => KeyAction::SpaceCadet(SpaceCadet::RightShiftParen),
                // The range guard bounds the offset to `0..=2`, so `2` is the only
                // remaining case (right-shift Enter).
                _ => KeyAction::SpaceCadet(SpaceCadet::RightShiftEnter),
            },
            AUTOCORRECT_KC_BASE..=AUTOCORRECT_KC_VALID_TOP => match self.0 - AUTOCORRECT_KC_BASE {
                0 => KeyAction::Autocorrect(AutocorrectAction::Toggle),
                1 => KeyAction::Autocorrect(AutocorrectAction::On),
                // The range guard bounds the offset to `0..=2`, so `2` is the only
                // remaining case (off).
                _ => KeyAction::Autocorrect(AutocorrectAction::Off),
            },
            UNICODE_CYCLE_KEYCODE => KeyAction::UnicodeCycle,
            UNICODE_MAP_BASE..=UNICODE_MAP_TOP => {
                KeyAction::UnicodeMap((self.0 - UNICODE_MAP_BASE) as u8)
            }
            MACRO_BASE..=MACRO_TOP => KeyAction::Macro((self.0 & ENTRY_INDEX_MASK) as u8),
            BOOT_KEYCODE => KeyAction::Boot,
            CONSUMER_BASE..=0xFFFF => KeyAction::Consumer(self.0 & CONSUMER_USAGE_MASK),
            _ => KeyAction::NoOp,
        }
    }
}

/// Momentary layer switch, `MO(layer)` — active while held. Free function so
/// keymaps read like `momentary_layer(1)`.
pub const fn momentary_layer(layer: u8) -> Keycode {
    Keycode::momentary(layer)
}

/// Activate-layer switch, `TO(layer)` — see [`Keycode::to_layer`]. Free function
/// so keymaps read like `to_layer(1)`.
pub const fn to_layer(layer: u8) -> Keycode {
    Keycode::to_layer(layer)
}

/// Toggle-layer switch, `TG(layer)` — see [`Keycode::toggle`]. Free function so
/// keymaps read like `toggle_layer(1)`.
pub const fn toggle_layer(layer: u8) -> Keycode {
    Keycode::toggle(layer)
}

/// Tap-or-hold layer switch, `TT(layer)` — see [`Keycode::tap_toggle`]. Free
/// function so keymaps read like `tap_toggle_layer(1)`.
pub const fn tap_toggle_layer(layer: u8) -> Keycode {
    Keycode::tap_toggle(layer)
}

/// One-shot layer switch, `OSL(layer)` — see [`Keycode::one_shot`]. Free function
/// so keymaps read like `one_shot_layer(1)`.
pub const fn one_shot_layer(layer: u8) -> Keycode {
    Keycode::one_shot(layer)
}

/// Mod-tap, `MT(mods, kc)` — see [`Keycode::mod_tap`]. Free function so keymaps and
/// tests read like `mod_tap(0x01, 0x04)` (hold Left Control, tap A).
pub const fn mod_tap(mods: u8, kc: u8) -> Keycode {
    Keycode::mod_tap(mods, kc)
}

/// Layer-tap, `LT(layer, kc)` — see [`Keycode::layer_tap`]. Free function so keymaps
/// and tests read like `layer_tap(1, 0x2C)`.
pub const fn layer_tap(layer: u8, kc: u8) -> Keycode {
    Keycode::layer_tap(layer, kc)
}

/// Tap-dance entry, `TD(index)` — see [`Keycode::tap_dance`]. Free function so
/// keymaps and tests read like `tap_dance(0)`.
pub const fn tap_dance(index: u8) -> Keycode {
    Keycode::tap_dance(index)
}

/// Dynamic-macro entry, `MACRO(index)` — see [`Keycode::macro_entry`]. Free
/// function so keymaps and tests read like `macro_keycode(0)`.
pub const fn macro_keycode(index: u8) -> Keycode {
    Keycode::macro_entry(index)
}

// ===========================================================================
// Keycode constants (USB HID keyboard/keypad usage page, 0x07)
//
// Named with the W3C `KeyboardEvent.code` vocabulary in SCREAMING_SNAKE
// (`KEY_A`, `ESCAPE`, `ARROW_UP`, `CONTROL_LEFT`). Each value is the HID usage,
// except `NONE`/`TRANSPARENT`, which are the two engine sentinels. Unused
// constants cost nothing in the final image.
// ===========================================================================

/// No-op (unbound key).
pub const NONE: Keycode = Keycode::NO;
/// Transparent (fall through to a lower layer).
pub const TRANSPARENT: Keycode = Keycode::TRNS;

// --- Letters ---
pub const KEY_A: Keycode = Keycode::from_usage(0x04);
pub const KEY_B: Keycode = Keycode::from_usage(0x05);
pub const KEY_C: Keycode = Keycode::from_usage(0x06);
pub const KEY_D: Keycode = Keycode::from_usage(0x07);
pub const KEY_E: Keycode = Keycode::from_usage(0x08);
pub const KEY_F: Keycode = Keycode::from_usage(0x09);
pub const KEY_G: Keycode = Keycode::from_usage(0x0A);
pub const KEY_H: Keycode = Keycode::from_usage(0x0B);
pub const KEY_I: Keycode = Keycode::from_usage(0x0C);
pub const KEY_J: Keycode = Keycode::from_usage(0x0D);
pub const KEY_K: Keycode = Keycode::from_usage(0x0E);
pub const KEY_L: Keycode = Keycode::from_usage(0x0F);
pub const KEY_M: Keycode = Keycode::from_usage(0x10);
pub const KEY_N: Keycode = Keycode::from_usage(0x11);
pub const KEY_O: Keycode = Keycode::from_usage(0x12);
pub const KEY_P: Keycode = Keycode::from_usage(0x13);
pub const KEY_Q: Keycode = Keycode::from_usage(0x14);
pub const KEY_R: Keycode = Keycode::from_usage(0x15);
pub const KEY_S: Keycode = Keycode::from_usage(0x16);
pub const KEY_T: Keycode = Keycode::from_usage(0x17);
pub const KEY_U: Keycode = Keycode::from_usage(0x18);
pub const KEY_V: Keycode = Keycode::from_usage(0x19);
pub const KEY_W: Keycode = Keycode::from_usage(0x1A);
pub const KEY_X: Keycode = Keycode::from_usage(0x1B);
pub const KEY_Y: Keycode = Keycode::from_usage(0x1C);
pub const KEY_Z: Keycode = Keycode::from_usage(0x1D);

// --- Number row ---
pub const DIGIT_1: Keycode = Keycode::from_usage(0x1E);
pub const DIGIT_2: Keycode = Keycode::from_usage(0x1F);
pub const DIGIT_3: Keycode = Keycode::from_usage(0x20);
pub const DIGIT_4: Keycode = Keycode::from_usage(0x21);
pub const DIGIT_5: Keycode = Keycode::from_usage(0x22);
pub const DIGIT_6: Keycode = Keycode::from_usage(0x23);
pub const DIGIT_7: Keycode = Keycode::from_usage(0x24);
pub const DIGIT_8: Keycode = Keycode::from_usage(0x25);
pub const DIGIT_9: Keycode = Keycode::from_usage(0x26);
pub const DIGIT_0: Keycode = Keycode::from_usage(0x27);

// --- Whitespace / editing ---
pub const ENTER: Keycode = Keycode::from_usage(0x28);
pub const ESCAPE: Keycode = Keycode::from_usage(0x29);
pub const BACKSPACE: Keycode = Keycode::from_usage(0x2A);
pub const TAB: Keycode = Keycode::from_usage(0x2B);
pub const SPACE: Keycode = Keycode::from_usage(0x2C);

// --- Symbols ---
pub const MINUS: Keycode = Keycode::from_usage(0x2D);
pub const EQUAL: Keycode = Keycode::from_usage(0x2E);
pub const BRACKET_LEFT: Keycode = Keycode::from_usage(0x2F);
pub const BRACKET_RIGHT: Keycode = Keycode::from_usage(0x30);
pub const BACKSLASH: Keycode = Keycode::from_usage(0x31);
/// Non-US `#`/`~`.
pub const INTL_HASH: Keycode = Keycode::from_usage(0x32);
pub const SEMICOLON: Keycode = Keycode::from_usage(0x33);
pub const QUOTE: Keycode = Keycode::from_usage(0x34);
pub const BACKQUOTE: Keycode = Keycode::from_usage(0x35);
pub const COMMA: Keycode = Keycode::from_usage(0x36);
pub const PERIOD: Keycode = Keycode::from_usage(0x37);
pub const SLASH: Keycode = Keycode::from_usage(0x38);
pub const CAPS_LOCK: Keycode = Keycode::from_usage(0x39);
/// Non-US `\`/`|`.
pub const INTL_BACKSLASH: Keycode = Keycode::from_usage(0x64);
/// Application / menu key.
pub const CONTEXT_MENU: Keycode = Keycode::from_usage(0x65);

// --- Function row ---
pub const F1: Keycode = Keycode::from_usage(0x3A);
pub const F2: Keycode = Keycode::from_usage(0x3B);
pub const F3: Keycode = Keycode::from_usage(0x3C);
pub const F4: Keycode = Keycode::from_usage(0x3D);
pub const F5: Keycode = Keycode::from_usage(0x3E);
pub const F6: Keycode = Keycode::from_usage(0x3F);
pub const F7: Keycode = Keycode::from_usage(0x40);
pub const F8: Keycode = Keycode::from_usage(0x41);
pub const F9: Keycode = Keycode::from_usage(0x42);
pub const F10: Keycode = Keycode::from_usage(0x43);
pub const F11: Keycode = Keycode::from_usage(0x44);
pub const F12: Keycode = Keycode::from_usage(0x45);

// --- Navigation / system ---
pub const PRINT_SCREEN: Keycode = Keycode::from_usage(0x46);
pub const SCROLL_LOCK: Keycode = Keycode::from_usage(0x47);
pub const PAUSE: Keycode = Keycode::from_usage(0x48);
pub const INSERT: Keycode = Keycode::from_usage(0x49);
pub const HOME: Keycode = Keycode::from_usage(0x4A);
pub const PAGE_UP: Keycode = Keycode::from_usage(0x4B);
pub const DELETE: Keycode = Keycode::from_usage(0x4C);
pub const END: Keycode = Keycode::from_usage(0x4D);
pub const PAGE_DOWN: Keycode = Keycode::from_usage(0x4E);
pub const ARROW_RIGHT: Keycode = Keycode::from_usage(0x4F);
pub const ARROW_LEFT: Keycode = Keycode::from_usage(0x50);
pub const ARROW_DOWN: Keycode = Keycode::from_usage(0x51);
pub const ARROW_UP: Keycode = Keycode::from_usage(0x52);

// --- Modifiers (HID usages 0xE0..=0xE7) ---
/// Left Control.
pub const CONTROL_LEFT: Keycode = Keycode::from_usage(0xE0);
/// Left Shift.
pub const SHIFT_LEFT: Keycode = Keycode::from_usage(0xE1);
/// Left Alt.
pub const ALT_LEFT: Keycode = Keycode::from_usage(0xE2);
/// Left Meta — the GUI / Super / Windows / Command key (matches QMK `KC_LGUI` / `KC_LCMD`).
pub const META_LEFT: Keycode = Keycode::from_usage(0xE3);
/// Right Control.
pub const CONTROL_RIGHT: Keycode = Keycode::from_usage(0xE4);
/// Right Shift.
pub const SHIFT_RIGHT: Keycode = Keycode::from_usage(0xE5);
/// Right Alt.
pub const ALT_RIGHT: Keycode = Keycode::from_usage(0xE6);
/// Right Meta — the GUI / Super / Windows / Command key (matches QMK `KC_RGUI` / `KC_RCMD`).
pub const META_RIGHT: Keycode = Keycode::from_usage(0xE7);

// ===========================================================================
// Consumer-control keycode constants (USB HID consumer usage page, 0x0C)
//
// Media transport, volume, brightness and the application launchers the Fn layer
// assigns, named with the W3C `KeyboardEvent.code` media vocabulary
// in SCREAMING_SNAKE (`AUDIO_VOLUME_UP`, `MEDIA_PLAY_PAUSE`, `BROWSER_SEARCH`).
// Each value is the HID *consumer* usage, encoded into the `0xC000` region by
// [`Keycode::consumer`]; the keymap engine routes them to the consumer-control
// HID interface built in [`crate::usb`], never the keyboard. The usage values
// match QMK's `quantum/keycode.h` consumer mappings.
// ===========================================================================

/// Mute (Consumer `0xE2`).
pub const AUDIO_VOLUME_MUTE: Keycode = Keycode::consumer(0xE2);
/// Volume up (Consumer `0xE9`, Volume Increment).
pub const AUDIO_VOLUME_UP: Keycode = Keycode::consumer(0xE9);
/// Volume down (Consumer `0xEA`, Volume Decrement).
pub const AUDIO_VOLUME_DOWN: Keycode = Keycode::consumer(0xEA);
/// Play / Pause (Consumer `0xCD`).
pub const MEDIA_PLAY_PAUSE: Keycode = Keycode::consumer(0xCD);
/// Next track (Consumer `0xB5`, Scan Next Track).
pub const MEDIA_TRACK_NEXT: Keycode = Keycode::consumer(0xB5);
/// Previous track (Consumer `0xB6`, Scan Previous Track).
pub const MEDIA_TRACK_PREVIOUS: Keycode = Keycode::consumer(0xB6);
/// Stop (Consumer `0xB7`).
pub const MEDIA_STOP: Keycode = Keycode::consumer(0xB7);
/// Display brightness up (Consumer `0x6F`, Display Brightness Increment).
pub const BRIGHTNESS_UP: Keycode = Keycode::consumer(0x6F);
/// Display brightness down (Consumer `0x70`, Display Brightness Decrement).
pub const BRIGHTNESS_DOWN: Keycode = Keycode::consumer(0x70);
/// Calculator — W3C `LaunchApp2` (Consumer `0x192`, AL Calculator).
pub const LAUNCH_APP2: Keycode = Keycode::consumer(0x192);
/// My Computer / file browser — W3C `LaunchApp1` (Consumer `0x194`, AL Local Machine Browser).
pub const LAUNCH_APP1: Keycode = Keycode::consumer(0x194);
/// Mail (Consumer `0x18A`, AL Email Reader).
pub const LAUNCH_MAIL: Keycode = Keycode::consumer(0x18A);
/// Web search (Consumer `0x221`, AC Search).
pub const BROWSER_SEARCH: Keycode = Keycode::consumer(0x221);
/// Web home (Consumer `0x223`, AC Home).
pub const BROWSER_HOME: Keycode = Keycode::consumer(0x223);
/// Media select — W3C `MediaSelect` (Consumer `0x183`, AL Consumer Control Configuration).
pub const MEDIA_SELECT: Keycode = Keycode::consumer(0x183);

// ===========================================================================
// Firmware-control keycodes
//
// Internal control codes with no HID usage; the keymap engine acts on them
// directly rather than emitting a report. Named in the keeberry behaviour
// vocabulary (`BOOTLOADER`, `LAYER_LOCK`, `LEADER`, `AUTO_SHIFT_*`, `AUTOCORRECT_*`,
// `CAPS_WORD`, `KEY_LOCK`, `REPEAT`, `ALT_REPEAT`, `GRAVE_ESCAPE`, `SPACE_CADET_*`,
// `UNICODE_MODE_CYCLE`); the firmware never relies on QMK's numbering.
// ===========================================================================

/// Reset into the `wb32-dfu` bootloader (QMK's `QK_BOOT`/`QK_BOOTLOADER`,
/// `0x7C00`). A press makes [`crate::keymap::compute_report`] call
/// [`crate::boot::bootloader_jump`], so mapping a key (e.g. a Fn combo) to it
/// enters DFU straight from the keyboard. See [`crate::boot`].
pub const BOOTLOADER: Keycode = Keycode::from_raw(BOOT_KEYCODE);

/// Layer lock (`LAYER_LOCK`, `0x5A00`). Mirrors QMK's `QK_LAYER_LOCK`: a press locks
/// the highest currently-active layer on so a momentary / one-shot / tap-toggle
/// layer survives its key lifting; pressing it again on the locked layer unlocks it.
/// [`crate::keymap::compute_report`] acts on it directly.
pub const LAYER_LOCK: Keycode = Keycode::layer_lock();

/// Auto-shift toggle (`AUTO_SHIFT_TOGGLE`, `0x5A01`). Flips the [`crate::timed`] auto-shift
/// enable flag; mirrors QMK's `AS_TOGG`.
pub const AUTO_SHIFT_TOGGLE: Keycode = Keycode::auto_shift(AutoShiftAction::Toggle);
/// Auto-shift on (`AUTO_SHIFT_ON`, `0x5A02`). Enables auto-shift; mirrors QMK's `AS_ON`.
pub const AUTO_SHIFT_ON: Keycode = Keycode::auto_shift(AutoShiftAction::On);
/// Auto-shift off (`AUTO_SHIFT_OFF`, `0x5A03`). Disables auto-shift; mirrors QMK's `AS_OFF`.
pub const AUTO_SHIFT_OFF: Keycode = Keycode::auto_shift(AutoShiftAction::Off);

/// Leader key (`LEADER`, `0x5A04`). A press opens a leader sequence the
/// [`crate::timed`] engine matches against the host-uploaded table; mirrors QMK's
/// `QK_LEADER`.
pub const LEADER: Keycode = Keycode::leader();

/// Autocorrect toggle (`AUTOCORRECT_TOGGLE`, `0x5A40`). Flips the autocorrect plugin's enable flag
/// ([`crate::features::autocorrect`]); mirrors QMK's `AC_TOGG`.
pub const AUTOCORRECT_TOGGLE: Keycode = Keycode::autocorrect(AutocorrectAction::Toggle);
/// Autocorrect on (`AUTOCORRECT_ON`, `0x5A41`). Enables autocorrect; mirrors QMK's `AC_ON`.
pub const AUTOCORRECT_ON: Keycode = Keycode::autocorrect(AutocorrectAction::On);
/// Autocorrect off (`AUTOCORRECT_OFF`, `0x5A42`). Disables autocorrect; mirrors QMK's `AC_OFF`.
pub const AUTOCORRECT_OFF: Keycode = Keycode::autocorrect(AutocorrectAction::Off);

// The remaining parameterless behaviour keycodes as top-level consts, so the firmware and
// the app GUI (`app/src/kcp/keycode.ts`) name the same controls by the same identifier.
/// Caps Word (`0x5A05`). A press holds Shift across a word until a non-word key ends it
/// ([`crate::features::caps_word`]).
pub const CAPS_WORD: Keycode = Keycode::caps_word();
/// Key Lock (`0x5A06`). A press arms key-lock so the next key latches held until pressed
/// again ([`crate::features::key_lock`]).
pub const KEY_LOCK: Keycode = Keycode::key_lock();
/// Repeat (`0x5A07`). A press re-emits the last key and the modifiers held with it
/// ([`crate::features::repeat_key`]).
pub const REPEAT: Keycode = Keycode::repeat();
/// Alternate Repeat (`0x5A08`). A press re-emits the last key's mapped alternate
/// ([`crate::features::repeat_key`]).
pub const ALT_REPEAT: Keycode = Keycode::alt_repeat();
/// Grave-Escape (`0x5A30`). Emits Escape, or grave / `~` while any Shift or GUI is held.
pub const GRAVE_ESCAPE: Keycode = Keycode::grave_escape();
/// Space-Cadet left paren (`0x5A31`): Left Shift held, tap emits `(`.
pub const SPACE_CADET_PAREN_LEFT: Keycode = Keycode::space_cadet(SpaceCadet::LeftShiftParen);
/// Space-Cadet right paren (`0x5A32`): Right Shift held, tap emits `)`.
pub const SPACE_CADET_PAREN_RIGHT: Keycode = Keycode::space_cadet(SpaceCadet::RightShiftParen);
/// Space-Cadet Enter (`0x5A33`): Right Shift held, tap emits Enter.
pub const SPACE_CADET_ENTER: Keycode = Keycode::space_cadet(SpaceCadet::RightShiftEnter);
/// Unicode mode-cycle (`0x5A50`). A press cycles the active OS Unicode input mode
/// ([`crate::features::unicode`]).
pub const UNICODE_MODE_CYCLE: Keycode = Keycode::unicode_cycle();

// ===========================================================================
// Mouse keycode constants (the `0x5500` region; no HID usage page of their own)
//
// USB HID mouse actions, resolved by [`crate::keymap::mouse_keys`] and turned into
// mouse reports by the [`crate::mouse`] accelerator. Named `MOUSE_*` in the
// keeberry vocabulary; the firmware never relies on QMK's numbering. They have no
// keyboard usage, so `compute_report` ignores them.
// ===========================================================================

/// Move the mouse pointer up.
pub const MOUSE_UP: Keycode = Keycode::mouse_key(MouseKey::Up);
/// Move the mouse pointer down.
pub const MOUSE_DOWN: Keycode = Keycode::mouse_key(MouseKey::Down);
/// Move the mouse pointer left.
pub const MOUSE_LEFT: Keycode = Keycode::mouse_key(MouseKey::Left);
/// Move the mouse pointer right.
pub const MOUSE_RIGHT: Keycode = Keycode::mouse_key(MouseKey::Right);
/// Mouse button 1 (left click).
pub const MOUSE_BUTTON_1: Keycode = Keycode::mouse_key(MouseKey::Btn1);
/// Mouse button 2 (right click).
pub const MOUSE_BUTTON_2: Keycode = Keycode::mouse_key(MouseKey::Btn2);
/// Mouse button 3 (middle click).
pub const MOUSE_BUTTON_3: Keycode = Keycode::mouse_key(MouseKey::Btn3);
/// Scroll the wheel up (away from the user).
pub const MOUSE_WHEEL_UP: Keycode = Keycode::mouse_key(MouseKey::WheelUp);
/// Scroll the wheel down (toward the user).
pub const MOUSE_WHEEL_DOWN: Keycode = Keycode::mouse_key(MouseKey::WheelDown);

// ===========================================================================
// Gamepad keycode constants (the `0x5600` region; no HID usage page of their own)
//
// DirectInput-style joystick actions, resolved by [`crate::keymap::gamepad_keys`]
// and turned into gamepad reports by the [`crate::gamepad`] decoder. Named
// `GAMEPAD_BUTTON_*` / `JOYSTICK_*` and host-1-indexed: `GAMEPAD_BUTTON_1` wraps the
// internal `Button(0)` and is reported as HID Button 1. They have no keyboard
// usage, so `compute_report` ignores them.
// ===========================================================================

/// Gamepad button 1 (HID Button 1).
pub const GAMEPAD_BUTTON_1: Keycode = Keycode::gamepad_key(GamepadKey::Button(0));
/// Gamepad button 2 (HID Button 2).
pub const GAMEPAD_BUTTON_2: Keycode = Keycode::gamepad_key(GamepadKey::Button(1));
/// Gamepad button 3 (HID Button 3).
pub const GAMEPAD_BUTTON_3: Keycode = Keycode::gamepad_key(GamepadKey::Button(2));
/// Gamepad button 4 (HID Button 4).
pub const GAMEPAD_BUTTON_4: Keycode = Keycode::gamepad_key(GamepadKey::Button(3));
/// Gamepad button 5 (HID Button 5).
pub const GAMEPAD_BUTTON_5: Keycode = Keycode::gamepad_key(GamepadKey::Button(4));
/// Gamepad button 6 (HID Button 6).
pub const GAMEPAD_BUTTON_6: Keycode = Keycode::gamepad_key(GamepadKey::Button(5));
/// Gamepad button 7 (HID Button 7).
pub const GAMEPAD_BUTTON_7: Keycode = Keycode::gamepad_key(GamepadKey::Button(6));
/// Gamepad button 8 (HID Button 8).
pub const GAMEPAD_BUTTON_8: Keycode = Keycode::gamepad_key(GamepadKey::Button(7));
/// Gamepad button 9 (HID Button 9).
pub const GAMEPAD_BUTTON_9: Keycode = Keycode::gamepad_key(GamepadKey::Button(8));
/// Gamepad button 10 (HID Button 10).
pub const GAMEPAD_BUTTON_10: Keycode = Keycode::gamepad_key(GamepadKey::Button(9));
/// Gamepad button 11 (HID Button 11).
pub const GAMEPAD_BUTTON_11: Keycode = Keycode::gamepad_key(GamepadKey::Button(10));
/// Gamepad button 12 (HID Button 12).
pub const GAMEPAD_BUTTON_12: Keycode = Keycode::gamepad_key(GamepadKey::Button(11));
/// Gamepad button 13 (HID Button 13).
pub const GAMEPAD_BUTTON_13: Keycode = Keycode::gamepad_key(GamepadKey::Button(12));
/// Gamepad button 14 (HID Button 14).
pub const GAMEPAD_BUTTON_14: Keycode = Keycode::gamepad_key(GamepadKey::Button(13));
/// Gamepad button 15 (HID Button 15).
pub const GAMEPAD_BUTTON_15: Keycode = Keycode::gamepad_key(GamepadKey::Button(14));
/// Gamepad button 16 (HID Button 16).
pub const GAMEPAD_BUTTON_16: Keycode = Keycode::gamepad_key(GamepadKey::Button(15));
/// Left-stick X axis to its negative end (−127).
pub const JOYSTICK_X_MINUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisXNeg);
/// Left-stick X axis to its positive end (+127).
pub const JOYSTICK_X_PLUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisXPos);
/// Left-stick Y axis to its negative end (−127).
pub const JOYSTICK_Y_MINUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisYNeg);
/// Left-stick Y axis to its positive end (+127).
pub const JOYSTICK_Y_PLUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisYPos);
/// Right-stick Z axis to its negative end (−127).
pub const JOYSTICK_Z_MINUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisZNeg);
/// Right-stick Z axis to its positive end (+127).
pub const JOYSTICK_Z_PLUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisZPos);
/// Right-stick Rz axis to its negative end (−127).
pub const JOYSTICK_RZ_MINUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisRzNeg);
/// Right-stick Rz axis to its positive end (+127).
pub const JOYSTICK_RZ_PLUS: Keycode = Keycode::gamepad_key(GamepadKey::AxisRzPos);
