// SPDX-License-Identifier: GPL-2.0-or-later
//! Persistent configuration: one versioned, CRC-protected blob holding the
//! **complete** device state, stored in the reserved flash
//! [`CONFIG_REGION`](crate::flash::CONFIG_REGION).
//!
//! User-editable settings are edited live in RAM over kcp and are otherwise lost
//! on reboot — among them the keymap ([`crate::keymap`]), the NKRO toggle
//! ([`crate::usb`]), the RGB state ([`crate::rgb`]), the SOCD pairs and key
//! overrides ([`crate::behavior`]) and the tap-dance / combo / macro tables
//! ([`crate::timed`]); the blob-layout table below is the authoritative, exhaustive
//! list of every persisted field. This module snapshots all of them into a single
//! fixed-layout blob, writes it to the reserved flash pages via the [`crate::flash`]
//! FMC driver, and restores every group at boot. The host triggers a save
//! explicitly (kcp `CONFIG.SAVE`); a bare live edit only updates RAM.
//!
//! # Blob layout (little-endian)
//!
//! | offset                | size              | field                          |
//! |-----------------------|-------------------|--------------------------------|
//! | [`MAGIC_OFF`]         | 4                 | [`MAGIC`]                      |
//! | [`VERSION_OFF`]       | 2                 | [`SCHEMA_VERSION`]             |
//! | [`NKRO_OFF`]          | 1                 | NKRO enabled flag              |
//! | [`RGB_OFF`]           | 7                 | mode,hue,sat,val,bright,on,speed|
//! | [`DEBOUNCE_OFF`]      | 2                 | debounce algorithm, interval   |
//! | [`KEYMAP_OFF`]        | [`KEYMAP_BYTES`]  | keymap `[layer][row][col]` u16 |
//! | [`SOCD_OFF`]          | [`SOCD_BYTES`]    | SOCD pairs                     |
//! | [`OVERRIDE_OFF`]      | [`OVERRIDE_BYTES`]| key overrides                  |
//! | [`TAPDANCE_OFF`]      | [`TAPDANCE_BYTES`]| tap-dance entries              |
//! | [`COMBO_OFF`]         | [`COMBO_BYTES`]   | combos                         |
//! | [`MACRO_OFF`]         | [`MACRO_BYTES`]   | macros (len + steps)           |
//! | [`TUNING_OFF`]        | [`TUNING_BYTES`]  | auto-shift, leader & tap-hold tuning |
//! | [`LEADER_OFF`]        | [`LEADER_BYTES`]  | leader sequence table          |
//! | [`LAYERCFG_OFF`]      | [`LAYERCFG_BYTES`]| default layer + tri-layer rule |
//! | [`ZONES_OFF`]         | [`ZONES_BYTES`]   | RGB zone table (`ZONE_CAP` × 12) |
//! | [`INDICATORS_OFF`]    | [`INDICATORS_BYTES`]| status-indicator overlay on/off |
//! | [`ENABLE_OFF`]        | [`ENABLE_BYTES`]  | feature-enable bitmap (u32 LE) |
//! | [`WLSPREF_OFF`]       | [`WLSPREF_BYTES`] | preferred wireless transport (`Devs` code) |
//! | [`CRC_OFF`]           | 4                 | CRC-32 over bytes `0..CRC_OFF` |
//!
//! Total [`BLOB_LEN`] bytes — also the worst case, because every table is fixed-
//! capacity, so a blob with every slot full and every macro at [`MAX_MACRO_STEPS`]
//! (crate::timed::MAX_MACRO_STEPS) steps is exactly this size. The CRC is the
//! software CRC-32/ISO-HDLC; a blob is accepted only if [`MAGIC`],
//! [`SCHEMA_VERSION`] **and** CRC all match — an **exact-match** check, never a
//! migration. An erased, partially written, wrong-version or corrupt region
//! therefore reads back as "no valid config" and the power-on defaults stand.
//!
//! # Why a `static` scratch, not a stack buffer
//!
//! The blob is ~4 KiB (`BLOB_LEN`). Materialising it on the stack on the kcp path would be
//! wasteful, so it is built in one [`SCRATCH`] static behind the project's
//! blocking-mutex/`RefCell` discipline. Crucially, no [`SCRATCH`] lock is ever
//! held across a flash erase/program: [`save`] builds the blob under one brief
//! lock, then copies each page out under a brief lock and runs the (millisecond-
//! scale, interrupt-disabling) FMC ops with the lock released — so interrupts are
//! re-enabled between pages exactly as the per-op flash driver already requires.

use core::cell::RefCell;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;

use crate::behavior::{self, KeyOverride, SocdMode};
use crate::features::{self, FeatureId};
use crate::flash;
use crate::keycode::Keycode;
use crate::keymap::{self, Keymap, LAYERS};
use crate::matrix::{self, NUM_COLS, NUM_ROWS};
use crate::rgb;
use crate::timed::{self, ComboCfg, LeaderCfg, MacroStep, TapDanceCfg, TapHoldTuning};
use crate::usb;
use crate::wireless;

/// Blob magic, `b"kbcf"` stored little-endian (keeberry config). Distinguishes a
/// written blob from erased flash (which reads back as all-`0x00`).
const MAGIC: u32 = 0x6663_626B;

/// On-flash blob schema version. Part of the **exact-match** validity check, not
/// a migration key: [`blob_valid`] rejects any other value outright, so a blob
/// from a different firmware layout is ignored and the defaults stand. Bump on
/// any layout change.
///
/// Exposed to the host over kcp ([`CMD_GET_DEVICE_INFO`](crate::kcp)) so the GUI
/// can tell whether a config it backed up is restorable into this firmware across
/// a flash — this is the version this firmware reads and writes.
///
/// Version 11 appended the preferred wireless transport (the auto-fallback target,
/// [`crate::wireless::preferred_wireless`]), so a USB unplug after reboot resumes the
/// user's wireless channel; version 10 replaced the standalone autocorrect enable flag
/// with the registry-wide
/// feature-enable bitmap (one persisted bit per [`crate::features::FeatureId`], holding
/// every feature's runtime master switch in a single word); version 9 grew the keymap
/// from eight to sixteen layers (the keymap blob doubled, shifting every group after it
/// up); version 8 appended the autocorrect enable flag; version 7 added the RGB zone table (the
/// per-zone link/independent/disabled effect state) and persisted the status-indicator
/// overlay flag (live-only before v7); version 6
/// added the per-combo flags byte and the layer-config block (the persistent `DF`
/// default layer and the tri-layer rule); version 5 grew the keymap to eight layers
/// and extended the runtime-tunable block with the mod-tap / layer-tap tuning (term,
/// interrupt flavours, retro, quick-tap); version 4 added the runtime-tunable block
/// (auto-shift enable + timeout, leader timeout) and the leader sequence table;
/// version 3 added the RGB animation speed byte and the matrix debounce block;
/// version 2 was the first full-config layout; version 1 the keymap-only blob it
/// replaced. The check is exact-match, so an older blob is ignored and the
/// power-on defaults stand — a clean, lossless fallback, never a partial restore.
pub const SCHEMA_VERSION: u16 = 11;

// === Blob field offsets (a fixed layout; every group at a compile-time base) ===

/// Offset of [`MAGIC`].
const MAGIC_OFF: usize = 0;
/// Offset of [`SCHEMA_VERSION`].
const VERSION_OFF: usize = 4;

/// Offset of the NKRO-enabled flag (`1` = NKRO, `0` = boot 6KRO).
const NKRO_OFF: usize = VERSION_OFF + 2;
/// Bytes the NKRO flag occupies.
const NKRO_BYTES: usize = 1;

/// Offset of the RGB block: `[mode, hue, sat, val, brightness, enabled, speed]`.
const RGB_OFF: usize = NKRO_OFF + NKRO_BYTES;
/// Bytes the RGB block occupies.
const RGB_BYTES: usize = 7;

/// Offset of the debounce block: `[algorithm, interval]` (matrix debounce config).
const DEBOUNCE_OFF: usize = RGB_OFF + RGB_BYTES;
/// Bytes the debounce block occupies.
const DEBOUNCE_BYTES: usize = 2;

/// Serialised keymap size: every `[layer][row][col]` cell is a little-endian
/// `u16` keycode.
pub const KEYMAP_BYTES: usize = LAYERS * NUM_ROWS * NUM_COLS * 2;
/// Offset of the keymap block.
const KEYMAP_OFF: usize = DEBOUNCE_OFF + DEBOUNCE_BYTES;

/// Bytes per serialised SOCD slot: `present(1) + a(2) + b(2) + mode(1)`.
const SOCD_SLOT_BYTES: usize = 6;
/// Offset of the SOCD table.
const SOCD_OFF: usize = KEYMAP_OFF + KEYMAP_BYTES;
/// Bytes the SOCD table occupies.
const SOCD_BYTES: usize = behavior::MAX_SOCD * SOCD_SLOT_BYTES;

/// Bytes per serialised key-override slot: `present(1) + trigger(2) +
/// trigger_mods(1) + replacement(2) + replacement_mods(1) + layer_mask(2) +
/// enabled(1)`.
const OVERRIDE_SLOT_BYTES: usize = 10;
/// Offset of the key-override table.
const OVERRIDE_OFF: usize = SOCD_OFF + SOCD_BYTES;
/// Bytes the key-override table occupies.
const OVERRIDE_BYTES: usize = behavior::MAX_OVERRIDES * OVERRIDE_SLOT_BYTES;

/// Bytes per serialised tap-dance slot: `present(1) + tap(2) + hold(2) +
/// double(2) + term(2)`.
const TAPDANCE_SLOT_BYTES: usize = 9;
/// Offset of the tap-dance table.
const TAPDANCE_OFF: usize = OVERRIDE_OFF + OVERRIDE_BYTES;
/// Bytes the tap-dance table occupies.
const TAPDANCE_BYTES: usize = timed::MAX_TAP_DANCE * TAPDANCE_SLOT_BYTES;

/// Bytes per serialised combo slot: `present(1) + len(1) +
/// keys(MAX_COMBO_KEYS·2) + action(2) + term(2) + flags(1)`.
const COMBO_SLOT_BYTES: usize = 2 + timed::MAX_COMBO_KEYS * 2 + 4 + 1;
/// Offset of the combo table.
const COMBO_OFF: usize = TAPDANCE_OFF + TAPDANCE_BYTES;
/// Bytes the combo table occupies.
const COMBO_BYTES: usize = timed::MAX_COMBO * COMBO_SLOT_BYTES;

/// Bytes per serialised macro step: `kc(2) + down(1) + delay(2)`.
const MACRO_STEP_BYTES: usize = 5;
/// Bytes per serialised macro slot: `len(1) + MAX_MACRO_STEPS·step`.
const MACRO_SLOT_BYTES: usize = 1 + timed::MAX_MACRO_STEPS * MACRO_STEP_BYTES;
/// Offset of the macro table.
const MACRO_OFF: usize = COMBO_OFF + COMBO_BYTES;
/// Bytes the macro table occupies.
const MACRO_BYTES: usize = timed::MAX_MACRO * MACRO_SLOT_BYTES;

/// Offset of the runtime-tunable block: `[auto_shift_enabled(1),
/// auto_shift_timeout(2), leader_timeout(2), tap_hold_term(2), tap_hold_flags(1),
/// quick_tap_term(2)]` (timeouts little-endian `u16`). The `tap_hold_flags` byte is
/// [`TapHoldTuning::flags_byte`], the same byte the kcp CONFIG TUNING group carries.
const TUNING_OFF: usize = MACRO_OFF + MACRO_BYTES;
/// Bytes the tunable block occupies.
const TUNING_BYTES: usize = 10;

/// Bytes per serialised leader slot: `len(1) + seq(MAX_LEADER_SEQ·2) + action(2)`.
const LEADER_SLOT_BYTES: usize = 1 + timed::MAX_LEADER_SEQ * 2 + 2;
/// Offset of the leader sequence table.
const LEADER_OFF: usize = TUNING_OFF + TUNING_BYTES;
/// Bytes the leader table occupies.
const LEADER_BYTES: usize = timed::MAX_LEADER * LEADER_SLOT_BYTES;

/// Offset of the layer-config block: `[default_layer, tri_enabled, tri_l1, tri_l2,
/// tri_l3]` — the persistent `DF` base layer and the tri-layer rule ([`crate::keymap`]).
/// Keymap-level (like the keymap and globals), not a registry feature.
const LAYERCFG_OFF: usize = LEADER_OFF + LEADER_BYTES;
/// Bytes the layer-config block occupies.
const LAYERCFG_BYTES: usize = 5;

/// Bytes per serialised zone slot: `flags(1) + mode(1) + h(1) + s(1) + v(1) +
/// bright(1) + speed(1) + start(2) + count(2) + sync_to(1)` — the live [`crate::rgb`]
/// zone fields. The `sync_to` byte is stored in the biased [`rgb::sync_to_wire`]
/// encoding (`0` = not synced, else `target + 1`), matching the GET_ZONE wire byte, so
/// a zeroed slot decodes to not-synced. (That byte was appended within schema v7, which
/// has not shipped as an external format; a pre-sync v7 blob fails the exact-length/CRC
/// check and falls back to the defaults, so no migration code is needed.)
const ZONE_SLOT_BYTES: usize = 12;
/// Offset of the RGB zone table: [`rgb::ZONE_CAP`] zones, each [`ZONE_SLOT_BYTES`],
/// mirroring the live zone table in [`crate::rgb`]. The schema-v7 block.
const ZONES_OFF: usize = LAYERCFG_OFF + LAYERCFG_BYTES;
/// Bytes the zone table occupies.
const ZONES_BYTES: usize = rgb::ZONE_CAP * ZONE_SLOT_BYTES;

/// Offset of the status-indicator overlay flag (`1` = overlay drawn). Persisted from
/// schema v7 so the [`crate::rgb`] indicator overlay survives a reboot.
const INDICATORS_OFF: usize = ZONES_OFF + ZONES_BYTES;
/// Bytes the indicator flag occupies.
const INDICATORS_BYTES: usize = 1;

/// Offset of the feature-enable bitmap (`u32` LE, bit `FeatureId as u32` = that
/// feature's runtime master switch). The schema-v10 block, persisted so every
/// feature's on/off choice survives a reboot; it folds in the former standalone
/// autocorrect flag. The default map enables every feature, so an older blob (rejected
/// by the exact-match check) leaves the whole registry on.
const ENABLE_OFF: usize = INDICATORS_OFF + INDICATORS_BYTES;
/// Bytes the enable bitmap occupies (a `u32`, so up to 32 features — the same bound
/// [`crate::features`] asserts on the highest `FeatureId` discriminant).
const ENABLE_BYTES: usize = 4;

/// Offset of the preferred wireless transport byte (a [`crate::wireless::Devs`] code:
/// the auto-fallback target when USB is unplugged). The schema-v11 block, persisted so
/// a reboot resumes the user's wireless channel; an unknown or USB code restores as the
/// 2.4 GHz default.
const WLSPREF_OFF: usize = ENABLE_OFF + ENABLE_BYTES;
/// Bytes the preferred-wireless field occupies.
const WLSPREF_BYTES: usize = 1;

// @scaffold:config-region — `just new-feature <Name> --kind config` (persisted) inserts a
// new feature's fixed region here: `const <NAME>_OFF: usize = WLSPREF_OFF + WLSPREF_BYTES;`
// and `const <NAME>_BYTES: usize = …;`, re-chains `CRC_OFF` onto it, and bumps
// `SCHEMA_VERSION` (a new layout invalidates every saved blob — validity is exact-match).
/// Offset of the trailing CRC-32 word (it covers every byte before it).
const CRC_OFF: usize = WLSPREF_OFF + WLSPREF_BYTES;

/// Total serialised blob length, and the worst case (every slot full).
pub const BLOB_LEN: usize = CRC_OFF + 4;

/// Number of flash pages the blob occupies (rounded up).
const PAGES: usize = BLOB_LEN.div_ceil(flash::PAGE_SIZE);

// The worst-case blob, and the pages it spans, must fit the reserved region.
const _: () = assert!(BLOB_LEN as u32 <= flash::CONFIG_REGION.end - flash::CONFIG_REGION.start);
const _: () = assert!(
    (PAGES * flash::PAGE_SIZE) as u32 <= flash::CONFIG_REGION.end - flash::CONFIG_REGION.start
);

/// Static scratch for the whole blob (built, programmed and validated here). In
/// `.bss`; accessed only on the kcp / boot path and never locked across a flash
/// op or an `.await` (see the module note on why a `static`).
static SCRATCH: Mutex<CriticalSectionRawMutex, RefCell<[u8; BLOB_LEN]>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new([0u8; BLOB_LEN]));

/// Static storage descriptor returned by [`storage_info`] for kcp
/// `CONFIG.GET_STORAGE_INFO`.
pub struct StorageInfo {
    /// Base address of the reserved region.
    pub base: u32,
    /// Size of the reserved region, in bytes.
    pub size: u32,
    /// Schema version of the stored blob (0 when none/invalid).
    pub version: u16,
    /// Whether a valid blob (magic + version + CRC) is currently stored.
    pub valid: bool,
}

/// Software CRC-32/ISO-HDLC (reflected, polynomial `0xEDB88320`, init/xorout
/// `0xFFFFFFFF`) over `data`. Table-free so it pulls in no `.rodata` and no
/// peripheral; the blob is small so the per-bit loop is cheap.
fn crc32(data: &[u8]) -> u32 {
    let mut crc: u32 = 0xFFFF_FFFF;
    for &byte in data {
        crc ^= byte as u32;
        for _ in 0..8 {
            // Branchless: subtract 1 from (crc & 1) to get 0x0 or 0xFFFFFFFF.
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

// === Little-endian field codecs ============================================

/// Read the `u16` at `off` in `buf`.
fn get_u16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

/// Read the `u32` at `off` in `buf`.
fn get_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// Write `v` little-endian at `off` in `buf`.
fn put_u16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// Write `v` little-endian at `off` in `buf`.
fn put_u32(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

/// Read a [`Keycode`] (raw little-endian `u16`) at `off`.
fn get_kc(buf: &[u8], off: usize) -> Keycode {
    Keycode::from_raw(get_u16(buf, off))
}

/// Write a [`Keycode`] (raw little-endian `u16`) at `off`.
fn put_kc(buf: &mut [u8], off: usize, kc: Keycode) {
    put_u16(buf, off, kc.raw());
}

// === Keymap (de)serialisation ==============================================

/// Serialise `km` into `out` (must be [`KEYMAP_BYTES`] long) as little-endian
/// `u16` cells in `[layer][row][col]` order.
fn serialize_keymap(km: &Keymap, out: &mut [u8]) {
    let mut i = 0;
    for layer in km.iter() {
        for row in layer.iter() {
            for kc in row.iter() {
                let b = kc.raw().to_le_bytes();
                out[i] = b[0];
                out[i + 1] = b[1];
                i += 2;
            }
        }
    }
}

/// Inverse of [`serialize_keymap`]: build a [`Keymap`] from `bytes` (must be at
/// least [`KEYMAP_BYTES`] long).
fn deserialize_keymap(bytes: &[u8]) -> Keymap {
    let mut km: Keymap = [[[Keycode::from_raw(0); NUM_COLS]; NUM_ROWS]; LAYERS];
    let mut i = 0;
    for layer in km.iter_mut() {
        for row in layer.iter_mut() {
            for kc in row.iter_mut() {
                *kc = Keycode::from_raw(u16::from_le_bytes([bytes[i], bytes[i + 1]]));
                i += 2;
            }
        }
    }
    km
}

// === Per-group snapshot (read live RAM state into the blob) =================

/// Snapshot the NKRO toggle, the RGB state (base effect, zone table and the
/// status-indicator flag), the matrix debounce config and the feature-enable bitmap into the blob.
fn serialize_globals(buf: &mut [u8]) {
    buf[NKRO_OFF] = usb::nkro_enabled() as u8;

    let (h, s, v) = rgb::hsv();
    buf[RGB_OFF] = rgb::mode();
    buf[RGB_OFF + 1] = h;
    buf[RGB_OFF + 2] = s;
    buf[RGB_OFF + 3] = v;
    buf[RGB_OFF + 4] = rgb::brightness();
    buf[RGB_OFF + 5] = rgb::enabled() as u8;
    buf[RGB_OFF + 6] = rgb::speed();

    buf[DEBOUNCE_OFF] = matrix::algorithm() as u8;
    buf[DEBOUNCE_OFF + 1] = matrix::interval();

    // RGB zone table (v7): the full ZONE_CAP zones, each ZONE_SLOT_BYTES, mirroring
    // the live table (flags + independent effect params + LED range).
    for id in 0..rgb::ZONE_CAP {
        let base = ZONES_OFF + id * ZONE_SLOT_BYTES;
        if let Some(z) = rgb::zone(id) {
            buf[base] = z.flags;
            buf[base + 1] = z.mode;
            buf[base + 2] = z.h;
            buf[base + 3] = z.s;
            buf[base + 4] = z.v;
            buf[base + 5] = z.bright;
            buf[base + 6] = z.speed;
            put_u16(buf, base + 7, z.start);
            put_u16(buf, base + 9, z.count);
            buf[base + 11] = rgb::sync_to_wire(z.sync_to);
        }
    }

    // Status-indicator overlay (v7): persisted so the choice survives a reboot.
    buf[INDICATORS_OFF] = rgb::indicators_enabled() as u8;

    // Feature-enable bitmap (v10): every feature's runtime master switch in one word.
    put_u32(buf, ENABLE_OFF, features::enabled_map());

    // Preferred wireless transport (v11): the auto-fallback target, so a USB unplug
    // after reboot resumes the user's channel.
    buf[WLSPREF_OFF] = wireless::preferred_wireless().code();
}

/// Snapshot the SOCD table — the [`Socd`](crate::behavior::Socd) feature's region,
/// written by [`save_feature`].
fn serialize_socd(buf: &mut [u8]) {
    for i in 0..behavior::MAX_SOCD {
        let base = SOCD_OFF + i * SOCD_SLOT_BYTES;
        if let Some(p) = behavior::socd_get(i) {
            buf[base] = 1;
            put_kc(buf, base + 1, p.a);
            put_kc(buf, base + 3, p.b);
            buf[base + 5] = p.mode as u8;
        }
    }
}

/// Snapshot the key-override table. The [`Overrides`](crate::behavior::Overrides)
/// feature's save hook.
fn serialize_overrides(buf: &mut [u8]) {
    for i in 0..behavior::MAX_OVERRIDES {
        let base = OVERRIDE_OFF + i * OVERRIDE_SLOT_BYTES;
        if let Some(o) = behavior::override_get(i) {
            buf[base] = 1;
            put_kc(buf, base + 1, o.trigger);
            buf[base + 3] = o.trigger_mods;
            put_kc(buf, base + 4, o.replacement);
            buf[base + 6] = o.replacement_mods;
            put_u16(buf, base + 7, o.layer_mask);
            buf[base + 9] = o.enabled as u8;
        }
    }
}

/// Snapshot the tap-dance table. Part of the [`Timed`](crate::timed::Timed)
/// feature's save hook.
fn serialize_tapdance(buf: &mut [u8]) {
    for i in 0..timed::MAX_TAP_DANCE {
        let base = TAPDANCE_OFF + i * TAPDANCE_SLOT_BYTES;
        if let Some(c) = timed::td_get(i) {
            buf[base] = 1;
            put_kc(buf, base + 1, c.tap);
            put_kc(buf, base + 3, c.hold);
            put_kc(buf, base + 5, c.double);
            put_u16(buf, base + 7, c.tap_term_ms);
        }
    }
}

/// Snapshot the combo table. Part of the [`Timed`](crate::timed::Timed) feature's
/// save hook.
fn serialize_combos(buf: &mut [u8]) {
    for i in 0..timed::MAX_COMBO {
        let base = COMBO_OFF + i * COMBO_SLOT_BYTES;
        if let Some(c) = timed::combo_get(i) {
            buf[base] = 1;
            buf[base + 1] = c.len;
            for (k, kc) in c.keys.iter().enumerate() {
                put_kc(buf, base + 2 + k * 2, *kc);
            }
            let action_off = base + 2 + timed::MAX_COMBO_KEYS * 2;
            put_kc(buf, action_off, c.action);
            put_u16(buf, action_off + 2, c.term_ms);
            buf[action_off + 4] = c.flags;
        }
    }
}

/// Snapshot the macro table (each slot: length byte + that many steps). Part of
/// the [`Timed`](crate::timed::Timed) feature's save hook.
fn serialize_macros(buf: &mut [u8]) {
    for i in 0..timed::MAX_MACRO {
        let base = MACRO_OFF + i * MACRO_SLOT_BYTES;
        // The length is reported alongside every step read; slot 0 always exists.
        let len = timed::macro_get_step(i, 0).map_or(0, |(_, l)| l);
        buf[base] = len;
        for s in 0..(len as usize).min(timed::MAX_MACRO_STEPS) {
            if let Some((step, _)) = timed::macro_get_step(i, s) {
                let so = base + 1 + s * MACRO_STEP_BYTES;
                put_kc(buf, so, step.kc);
                buf[so + 2] = step.down as u8;
                put_u16(buf, so + 3, step.delay_ms);
            }
        }
    }
}

/// Snapshot the runtime tunables (auto-shift enable + timeout, leader timeout, and the
/// mod-tap / layer-tap tuning). Part of the [`Timed`](crate::timed::Timed) feature's
/// save hook.
fn serialize_tunables(buf: &mut [u8]) {
    let (as_on, as_timeout) = timed::autoshift_get();
    buf[TUNING_OFF] = as_on as u8;
    put_u16(buf, TUNING_OFF + 1, as_timeout);
    put_u16(buf, TUNING_OFF + 3, timed::leader_timeout_ms());

    let th = timed::taphold_get();
    put_u16(buf, TUNING_OFF + 5, th.term_ms);
    buf[TUNING_OFF + 7] = th.flags_byte();
    put_u16(buf, TUNING_OFF + 8, th.quick_tap_term_ms);
}

/// Snapshot the layer configuration (the persistent `DF` default layer and the
/// tri-layer rule). Keymap-level — written inline by [`build_blob`] like the keymap,
/// not via a registry feature.
fn serialize_layercfg(buf: &mut [u8]) {
    let (tri_on, l1, l2, l3) = keymap::tri_layer();
    buf[LAYERCFG_OFF] = keymap::default_layer();
    buf[LAYERCFG_OFF + 1] = tri_on as u8;
    buf[LAYERCFG_OFF + 2] = l1;
    buf[LAYERCFG_OFF + 3] = l2;
    buf[LAYERCFG_OFF + 4] = l3;
}

/// Snapshot the leader sequence table (each slot: length byte + sequence + action).
/// Part of the [`Timed`](crate::timed::Timed) feature's save hook.
fn serialize_leader(buf: &mut [u8]) {
    for i in 0..timed::MAX_LEADER {
        let base = LEADER_OFF + i * LEADER_SLOT_BYTES;
        if let Some(c) = timed::leader_get(i) {
            buf[base] = c.len;
            for (k, kc) in c.seq.iter().enumerate() {
                put_kc(buf, base + 1 + k * 2, *kc);
            }
            put_kc(buf, base + 1 + timed::MAX_LEADER_SEQ * 2, c.action);
        }
    }
}

// === Per-group restore (write the blob back into live RAM state) ============

/// Restore the NKRO toggle, the RGB state (base effect, zone table and the
/// status-indicator flag) and the matrix debounce config from the blob.
fn deserialize_globals(buf: &[u8]) {
    usb::set_nkro(buf[NKRO_OFF] != 0);

    // The stored mode came from `rgb::mode()`, so it is always in range for a
    // CRC-valid blob; `set_mode` ignores an (impossible) bad value, leaving the
    // default — fail-safe rather than fatal.
    let _ = rgb::set_mode(buf[RGB_OFF]);
    rgb::set_hsv(buf[RGB_OFF + 1], buf[RGB_OFF + 2], buf[RGB_OFF + 3]);
    rgb::set_brightness(buf[RGB_OFF + 4]);
    rgb::set_enabled(buf[RGB_OFF + 5] != 0);
    rgb::set_speed(buf[RGB_OFF + 6]);

    // Likewise the debounce config was written by `set_debounce`, so it is always a
    // valid algorithm + non-zero interval for a CRC-valid blob; `set_debounce`
    // ignores an (impossible) bad value, keeping the current setting — fail-safe.
    let _ = matrix::set_debounce(buf[DEBOUNCE_OFF], buf[DEBOUNCE_OFF + 1]);

    // RGB zone table (v7). The stored fields came from `set_zone`/`set_zone_range`/
    // `set_zone_sync`, so they are in range (and the sync graph acyclic) for a CRC-valid
    // blob; each setter ignores an (impossible) bad value, leaving the default —
    // fail-safe rather than fatal. The range setter is the bounds-only one (overlap is
    // a host-resize concern), so an already-disjoint saved table replays verbatim; and
    // `set_zone`'s enable guard only fires on an off->on transition, which a boot
    // restore (every zone starts enabled) never triggers, so it too replays verbatim.
    for id in 0..rgb::ZONE_CAP {
        let base = ZONES_OFF + id * ZONE_SLOT_BYTES;
        let _ = rgb::set_zone_range(id, get_u16(buf, base + 7), get_u16(buf, base + 9));
        let _ = rgb::set_zone(
            id,
            buf[base],
            buf[base + 1],
            (buf[base + 2], buf[base + 3], buf[base + 4]),
            buf[base + 5],
            buf[base + 6],
        );
        let _ = rgb::set_zone_sync(id, rgb::sync_from_wire(buf[base + 11]));
    }
    rgb::set_indicators(buf[INDICATORS_OFF] != 0);

    // Feature-enable bitmap (v10): restore every feature's runtime master switch.
    // Always-on features are forced on by `set_enabled_map`, so a stale word can never
    // strand the structural core off.
    features::set_enabled_map(get_u32(buf, ENABLE_OFF));

    // Preferred wireless transport (v11). An unknown or USB code leaves the 2.4 GHz
    // default — fail-safe, like the other restores.
    wireless::set_preferred_wireless_code(buf[WLSPREF_OFF]);
}

/// Restore the SOCD table (cleared first, so absent slots end empty) — the
/// [`Socd`](crate::behavior::Socd) feature's region, restored by [`load_feature`].
fn deserialize_socd(buf: &[u8]) {
    behavior::socd_clear_all();
    for i in 0..behavior::MAX_SOCD {
        let base = SOCD_OFF + i * SOCD_SLOT_BYTES;
        if buf[base] != 0 {
            if let Some(mode) = SocdMode::from_u8(buf[base + 5]) {
                behavior::socd_set(i, get_kc(buf, base + 1), get_kc(buf, base + 3), mode);
            }
        }
    }
}

/// Restore the key-override table. The [`Overrides`](crate::behavior::Overrides)
/// feature's load hook.
fn deserialize_overrides(buf: &[u8]) {
    behavior::override_clear_all();
    for i in 0..behavior::MAX_OVERRIDES {
        let base = OVERRIDE_OFF + i * OVERRIDE_SLOT_BYTES;
        if buf[base] != 0 {
            behavior::override_set(
                i,
                KeyOverride {
                    trigger: get_kc(buf, base + 1),
                    trigger_mods: buf[base + 3],
                    replacement: get_kc(buf, base + 4),
                    replacement_mods: buf[base + 6],
                    layer_mask: get_u16(buf, base + 7),
                    enabled: buf[base + 9] != 0,
                },
            );
        }
    }
}

/// Restore the tap-dance table. Part of the [`Timed`](crate::timed::Timed)
/// feature's load hook.
fn deserialize_tapdance(buf: &[u8]) {
    timed::td_clear_all();
    for i in 0..timed::MAX_TAP_DANCE {
        let base = TAPDANCE_OFF + i * TAPDANCE_SLOT_BYTES;
        if buf[base] != 0 {
            timed::td_set(
                i,
                TapDanceCfg {
                    tap: get_kc(buf, base + 1),
                    hold: get_kc(buf, base + 3),
                    double: get_kc(buf, base + 5),
                    tap_term_ms: get_u16(buf, base + 7),
                },
            );
        }
    }
}

/// Restore the combo table. Part of the [`Timed`](crate::timed::Timed) feature's
/// load hook.
fn deserialize_combos(buf: &[u8]) {
    timed::combo_clear_all();
    for i in 0..timed::MAX_COMBO {
        let base = COMBO_OFF + i * COMBO_SLOT_BYTES;
        if buf[base] != 0 {
            let mut keys = [Keycode::from_raw(0); timed::MAX_COMBO_KEYS];
            for (k, kc) in keys.iter_mut().enumerate() {
                *kc = get_kc(buf, base + 2 + k * 2);
            }
            let action_off = base + 2 + timed::MAX_COMBO_KEYS * 2;
            timed::combo_set(
                i,
                ComboCfg {
                    keys,
                    len: buf[base + 1],
                    action: get_kc(buf, action_off),
                    term_ms: get_u16(buf, action_off + 2),
                    flags: buf[action_off + 4],
                },
            );
        }
    }
}

/// Restore the macro table (each slot rebuilt by setting its steps in order,
/// which grows the macro's length to cover them). Part of the
/// [`Timed`](crate::timed::Timed) feature's load hook.
fn deserialize_macros(buf: &[u8]) {
    timed::macro_clear_all();
    for i in 0..timed::MAX_MACRO {
        let base = MACRO_OFF + i * MACRO_SLOT_BYTES;
        let len = (buf[base] as usize).min(timed::MAX_MACRO_STEPS);
        for s in 0..len {
            let so = base + 1 + s * MACRO_STEP_BYTES;
            timed::macro_set_step(
                i,
                s,
                MacroStep {
                    kc: get_kc(buf, so),
                    down: buf[so + 2] != 0,
                    delay_ms: get_u16(buf, so + 3),
                },
            );
        }
    }
}

// === Blob assembly / validation ============================================

/// Restore the runtime tunables (auto-shift enable + timeout, leader timeout, and the
/// mod-tap / layer-tap tuning). Part of the [`Timed`](crate::timed::Timed) feature's
/// load hook.
fn deserialize_tunables(buf: &[u8]) {
    timed::autoshift_set(buf[TUNING_OFF] != 0, get_u16(buf, TUNING_OFF + 1));
    timed::leader_set_timeout(get_u16(buf, TUNING_OFF + 3));
    timed::taphold_set(TapHoldTuning::from_parts(
        get_u16(buf, TUNING_OFF + 5),
        buf[TUNING_OFF + 7],
        get_u16(buf, TUNING_OFF + 8),
    ));
}

/// Restore the leader sequence table (cleared first, so absent slots end empty).
/// Part of the [`Timed`](crate::timed::Timed) feature's load hook.
fn deserialize_leader(buf: &[u8]) {
    timed::leader_clear_all();
    for i in 0..timed::MAX_LEADER {
        let base = LEADER_OFF + i * LEADER_SLOT_BYTES;
        let len = buf[base];
        if len != 0 {
            let mut seq = [Keycode::from_raw(0); timed::MAX_LEADER_SEQ];
            for (k, kc) in seq.iter_mut().enumerate() {
                *kc = get_kc(buf, base + 1 + k * 2);
            }
            timed::leader_set(
                i,
                LeaderCfg {
                    seq,
                    len,
                    action: get_kc(buf, base + 1 + timed::MAX_LEADER_SEQ * 2),
                },
            );
        }
    }
}

/// Restore the layer configuration (the persistent `DF` default layer and the
/// tri-layer rule). Keymap-level, applied inline by [`restore_blob`]. The keymap
/// setters validate, so an (impossible for a CRC-valid blob) out-of-range value is
/// ignored, leaving the default — fail-safe rather than fatal.
fn deserialize_layercfg(buf: &[u8]) {
    keymap::set_default_layer(buf[LAYERCFG_OFF]);
    keymap::set_tri_layer(
        buf[LAYERCFG_OFF + 1] != 0,
        buf[LAYERCFG_OFF + 2],
        buf[LAYERCFG_OFF + 3],
        buf[LAYERCFG_OFF + 4],
    );
}

/// Snapshot the feature identified by `id` into its fixed-offset blob region(s):
/// the persistence half of a [`Feature`](crate::features::Feature)'s `on_save`,
/// dispatched by its [`FeatureId`]. This id→region map is the fixed-offset analogue
/// of a tagged blob — the offsets and bytes are unchanged, so a saved config stays
/// byte-identical — and is the one place that turns into a tag writer if the blob
/// later moves to TLV.
pub(crate) fn save_feature(id: FeatureId, buf: &mut [u8]) {
    match id {
        FeatureId::Socd => serialize_socd(buf),
        FeatureId::Overrides => serialize_overrides(buf),
        FeatureId::Timed => {
            serialize_tapdance(buf);
            serialize_combos(buf);
            serialize_macros(buf);
            serialize_tunables(buf);
            serialize_leader(buf);
        }
        // The behaviour plugins (caps-word, key-lock, repeat, one-shot-mod) keep only
        // transient runtime state, and the Unicode plugin's codepoint map is deliberately
        // RAM-only (re-uploaded by the host on connect, no schema field), so all persist
        // nothing — their `on_save` is the no-op default and never reaches here. Every
        // feature's enable bit (autocorrect included) rides the globals block's enable
        // bitmap (`ENABLE_OFF`), saved centrally, not via its `on_save`.
        FeatureId::CapsWord
        | FeatureId::KeyLock
        | FeatureId::RepeatKey
        | FeatureId::OneShotMod
        | FeatureId::Autocorrect
        | FeatureId::Unicode => {}
    }
}

/// Restore the feature identified by `id` from its fixed-offset blob region(s): the
/// load half, dispatched by [`FeatureId`]. Each table is cleared before its present
/// slots are applied (see the `deserialize_*` functions).
pub(crate) fn load_feature(id: FeatureId, buf: &[u8]) {
    match id {
        FeatureId::Socd => deserialize_socd(buf),
        FeatureId::Overrides => deserialize_overrides(buf),
        FeatureId::Timed => {
            deserialize_tapdance(buf);
            deserialize_combos(buf);
            deserialize_macros(buf);
            deserialize_tunables(buf);
            deserialize_leader(buf);
        }
        // No per-feature blob region for the behaviour plugins or the RAM-only Unicode
        // plugin (see `save_feature`); autocorrect's enable byte is restored inline by
        // `deserialize_globals`.
        FeatureId::CapsWord
        | FeatureId::KeyLock
        | FeatureId::RepeatKey
        | FeatureId::OneShotMod
        | FeatureId::Autocorrect
        | FeatureId::Unicode => {}
    }
}

/// Build the complete blob (header + every group + CRC) into `buf`.
///
/// `buf` is zeroed first so the result is a pure function of the live state:
/// absent table slots serialise as a deterministic `present = 0` with cleared
/// data. Reads live RAM through the existing group accessors.
fn build_blob(buf: &mut [u8]) {
    buf.fill(0);
    put_u32(buf, MAGIC_OFF, MAGIC);
    put_u16(buf, VERSION_OFF, SCHEMA_VERSION);

    serialize_globals(buf);
    serialize_keymap(&keymap::snapshot(), &mut buf[KEYMAP_OFF..KEYMAP_OFF + KEYMAP_BYTES]);
    // The SOCD, override and timed-engine regions are owned by the registry
    // features; each writes its own fixed-offset region (the `serialize_*`
    // functions above), so the blob is byte-identical regardless of registry
    // order. Globals, the keymap and the layer config are not features and stay inline.
    features::run_on_save(buf);
    serialize_layercfg(buf);

    let crc = crc32(&buf[..CRC_OFF]);
    put_u32(buf, CRC_OFF, crc);
}

/// Apply a validated blob to every group's live RAM state, in layout order. Each
/// table is cleared before its present slots are applied, so the restore is a
/// deterministic, idempotent overwrite (correct whether the tables start empty,
/// as at boot, or already populated).
fn restore_blob(buf: &[u8]) {
    deserialize_globals(buf);
    keymap::load_into_ram(deserialize_keymap(&buf[KEYMAP_OFF..KEYMAP_OFF + KEYMAP_BYTES]));
    // Each feature clears and rebuilds its own table from its fixed-offset region
    // (the `deserialize_*` functions above). Ungated by `active()` so a stored
    // table is restored even though the feature is idle until it is.
    features::run_on_load(buf);
    deserialize_layercfg(buf);
}

/// Whether `buf` (at least [`BLOB_LEN`] long) holds a valid stored config: the
/// **exact** magic and schema version, and a CRC matching everything before the
/// CRC word. Any mismatch means "no valid config" — defaults stand.
fn blob_valid(buf: &[u8]) -> bool {
    get_u32(buf, MAGIC_OFF) == MAGIC
        && get_u16(buf, VERSION_OFF) == SCHEMA_VERSION
        && get_u32(buf, CRC_OFF) == crc32(&buf[..CRC_OFF])
}

// === Public API ============================================================

/// Snapshot the complete live device state and persist it to [`CONFIG_REGION`].
///
/// Builds the versioned, CRC-protected blob in the [`SCRATCH`] static, then
/// erases and programs each page it spans through the [`crate::flash`] FMC driver
/// (the lock released around every op, so interrupts breathe between pages), and
/// finally reads the region back and revalidates (magic + version + CRC) as an
/// end-to-end check on top of the per-page read-back the driver already does.
/// Returns `Err(())` on any flash error or if the read-back fails to validate;
/// the live RAM state is untouched either way (a save only reads it).
///
/// [`CONFIG_REGION`]: crate::flash::CONFIG_REGION
pub fn save() -> Result<(), ()> {
    // Phase 1 — assemble the blob (pure memory; brief critical section).
    SCRATCH.lock(|cell| build_blob(&mut cell.borrow_mut()[..]));

    // Phase 2 — erase + program one page at a time, copying each page out under a
    // brief lock and running the FMC ops with no lock held (and a zero-padded
    // final page).
    for p in 0..PAGES {
        let mut page = [0u8; flash::PAGE_SIZE];
        SCRATCH.lock(|cell| {
            let buf = cell.borrow();
            let start = p * flash::PAGE_SIZE;
            let end = core::cmp::min(start + flash::PAGE_SIZE, BLOB_LEN);
            page[..end - start].copy_from_slice(&buf[start..end]);
        });

        let addr = flash::CONFIG_REGION.start + (p * flash::PAGE_SIZE) as u32;
        // SAFETY: `addr` is page-aligned within CONFIG_REGION by construction;
        // `flash` additionally hard-asserts the bound before any FMC access.
        unsafe {
            flash::erase_page(addr)?;
            flash::program_page(addr, &page)?;
        }
    }

    // Phase 3 — read the region back and revalidate the assembled blob.
    let ok = SCRATCH.lock(|cell| {
        let mut buf = cell.borrow_mut();
        flash::read(flash::CONFIG_REGION.start, &mut buf[..]);
        blob_valid(&buf[..])
    });
    if ok {
        Ok(())
    } else {
        Err(())
    }
}

/// Restore the persisted configuration into live RAM if a valid blob is stored.
///
/// Reads [`CONFIG_REGION`](crate::flash::CONFIG_REGION) into [`SCRATCH`],
/// validates it (magic + version + CRC) and, if valid, restores every group's RAM
/// state and returns `true`. Otherwise leaves the power-on defaults in place and
/// returns `false`. Read-only with respect to flash; the matching boot path for
/// [`save`].
pub fn restore() -> bool {
    // Seed the feature-enable bitmap with the factory defaults first, so the
    // no-valid-blob path below leaves every feature at its default; a valid blob then
    // overwrites the map in `restore_blob` (via `deserialize_globals`). The preferred
    // wireless transport needs no such seed here: its static default (2.4 GHz) already
    // stands and nothing mutates it before this boot-time restore.
    features::init_enabled();
    SCRATCH.lock(|cell| {
        let mut buf = cell.borrow_mut();
        flash::read(flash::CONFIG_REGION.start, &mut buf[..]);
        if blob_valid(&buf[..]) {
            restore_blob(&buf[..]);
            true
        } else {
            false
        }
    })
}

/// Reset every group's live RAM state to the firmware power-on defaults: the
/// default keymap, boot 6KRO, the default RGB state, the default matrix debounce
/// config, the default runtime tunables (auto-shift off, default timeouts), the
/// default layer config (base layer 0, tri-layer off), empty SOCD / override /
/// tap-dance / combo / macro / leader tables, every feature switched back on, and the
/// default preferred wireless transport (2.4 GHz).
/// RAM-only — the host calls [`save`] afterward to persist the cleared state. Backs
/// the kcp `CONFIG.RESET` operation.
pub fn reset_to_defaults() {
    keymap::load_into_ram(keymap::DEFAULT_KEYMAP);
    usb::set_nkro(false);
    rgb::reset_defaults();
    matrix::reset_debounce_defaults();
    behavior::socd_clear_all();
    behavior::override_clear_all();
    timed::td_clear_all();
    timed::combo_clear_all();
    timed::macro_clear_all();
    timed::reset_tunables();
    keymap::reset_layer_config();
    features::init_enabled();
    wireless::reset_preferred_wireless();
}

/// Describe the persistence region and the currently stored blob, for kcp
/// `CONFIG.GET_STORAGE_INFO`. Reads the region back through [`SCRATCH`].
pub fn storage_info() -> StorageInfo {
    SCRATCH.lock(|cell| {
        let mut buf = cell.borrow_mut();
        flash::read(flash::CONFIG_REGION.start, &mut buf[..]);
        let valid = blob_valid(&buf[..]);
        StorageInfo {
            base: flash::CONFIG_REGION.start,
            size: flash::CONFIG_REGION.end - flash::CONFIG_REGION.start,
            version: if valid { get_u16(&buf[..], VERSION_OFF) } else { 0 },
            valid,
        }
    })
}
