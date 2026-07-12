// SPDX-License-Identifier: GPL-2.0-or-later
//! Resource allocation by reading the firmware's own tables.
//!
//! The four allocation axes a non-trivial feature must claim a free slot in — the
//! [`FeatureId`] discriminant, the kcp group nibble, the keycode window and the config-blob
//! region — are each guarded by a build-time `assert!` in the firmware, so the firmware
//! source *is* the single source of truth. This module parses those tables (rather than
//! keeping a second copy that could drift) and hands the next-free slot to the wiring, so the
//! scaffolder and the asserts can never disagree: a bad pick is a build error, not a silent bug.

/// The slots allocated for one new feature, read live from the firmware sources.
pub struct Resources {
    /// Next contiguous [`FeatureId`] discriminant (`max + 1`).
    pub feature_id: u8,
    /// The currently-highest `FeatureId` variant name — the one the `< 32` enable-bitmap
    /// guard references today, which the scaffolder re-points at the new (now highest) variant.
    pub prev_highest: String,
    /// The lowest free kcp group nibble (`0xB`/`0xC`/`0xE`), for a `config` feature.
    pub nibble: Option<u8>,
    /// The next free nibble after [`nibble`](Self::nibble), printed in the checklist as the
    /// stand-in to re-point the "unknown capability bit" example test onto.
    pub next_free_nibble: Option<u8>,
    /// The current config-blob `CRC_OFF` offset expression, which a new persisted region
    /// chains onto (the new region takes this offset; `CRC_OFF` re-chains past the region).
    pub crc_expr: String,
    /// The current `SCHEMA_VERSION` (a persisted region bumps it by one).
    pub schema_version: u16,
    /// The capabilities mask today and with the new group's bit set, for the checklist's
    /// "update the protocol-surface snapshot to 0x…" hint.
    pub caps_old: u32,
    pub caps_new: u32,
    /// A suggested free keycode window base (`0xNN00`), for a `keycode` feature.
    pub keycode_window: Option<u16>,
}

impl Resources {
    /// Allocate from the live firmware sources for a feature of `kind`.
    pub fn allocate(
        mod_rs: &str,
        kcp_rs: &str,
        config_rs: &str,
        keycode_rs: &str,
    ) -> Result<Self, String> {
        let (feature_id, prev_highest) = next_feature_id(mod_rs)?;

        let used = used_group_nibbles(kcp_rs)?;
        let free: Vec<u8> = [0xB, 0xC, 0xE].into_iter().filter(|n| !used.contains(n)).collect();
        let caps_old = used.iter().fold(0u32, |m, &n| m | (1 << n));

        Ok(Self {
            feature_id,
            prev_highest,
            nibble: free.first().copied(),
            next_free_nibble: free.get(1).copied(),
            caps_old,
            caps_new: caps_old | free.first().map(|&n| 1u32 << n).unwrap_or(0),
            crc_expr: crc_expr(config_rs)?,
            schema_version: schema_version(config_rs)?,
            keycode_window: free_keycode_window(keycode_rs),
        })
    }
}

/// Parse `enum FeatureId { … }` and return `(max_discriminant + 1, name_of_max)`.
fn next_feature_id(mod_rs: &str) -> Result<(u8, String), String> {
    let body = between(mod_rs, "pub enum FeatureId {", "\n}")
        .ok_or("could not find `pub enum FeatureId` in features/mod.rs")?;
    let mut best: Option<(u8, String)> = None;
    for line in body.lines() {
        // A variant line is `    <Name> = <n>,`.
        let line = line.trim().trim_end_matches(',');
        if let Some((name, val)) = line.split_once(" = ") {
            if let (true, Ok(n)) = (is_ident(name), val.trim().parse::<u8>()) {
                if best.as_ref().is_none_or(|(b, _)| n > *b) {
                    best = Some((n, name.to_string()));
                }
            }
        }
    }
    let (max, name) = best.ok_or("no `FeatureId` variants parsed from features/mod.rs")?;
    Ok((max + 1, name))
}

/// Collect the group nibbles already assigned in `pub mod group { … }`. Errors if the module
/// can't be located or yields no nibbles: an unparsable table would otherwise read as "every
/// nibble free" and hand `0xB` to a feature on top of an existing group — a silent collision the
/// firmware's build-time asserts would only catch later. A loud failure here says the parser, not
/// the allocation, drifted.
fn used_group_nibbles(kcp_rs: &str) -> Result<Vec<u8>, String> {
    let body = between(kcp_rs, "pub mod group {", "\n}")
        .ok_or("could not find `pub mod group { … }` in kcp.rs — the group table moved")?;
    let mut out = Vec::new();
    for line in body.lines() {
        // A group line is `    pub const <NAME>: u8 = 0x<nibble>;`.
        if let Some(hex) = line.split("= 0x").nth(1) {
            if let Ok(n) = u8::from_str_radix(hex.trim().trim_end_matches(';'), 16) {
                out.push(n);
            }
        }
    }
    if out.is_empty() {
        return Err("parsed no nibbles from `pub mod group` in kcp.rs — the table shape drifted".into());
    }
    Ok(out)
}

/// The right-hand side of `const CRC_OFF: usize = <expr>;` in config.rs.
fn crc_expr(config_rs: &str) -> Result<String, String> {
    between(config_rs, "const CRC_OFF: usize = ", ";")
        .map(|s| s.trim().to_string())
        .ok_or_else(|| "could not find `const CRC_OFF` in config.rs".to_string())
}

/// The `SCHEMA_VERSION` value in config.rs.
fn schema_version(config_rs: &str) -> Result<u16, String> {
    between(config_rs, "pub const SCHEMA_VERSION: u16 = ", ";")
        .and_then(|s| s.trim().parse().ok())
        .ok_or_else(|| "could not parse `SCHEMA_VERSION` in config.rs".to_string())
}

/// The lowest free `0xNN00` keycode window with `NN` in `0x78..=0xBF` (the conventional
/// single-page band just past the dynamic-macro region, below the consumer block), judged against
/// *every* assigned keycode constant — region bases, singletons (`BOOT_KEYCODE = 0x7C00`) and
/// block tops alike, not just the `_BASE` regions. Scanning only the bases missed `0x7C`, so a
/// drifting table could place a window over the bootloader code; counting each constant's high
/// byte keeps the suggestion collision-free. Mask/offset constants land outside the band (their
/// high byte is `0x00`) and computed `= EXPR` constants carry no literal, so neither perturbs it.
fn free_keycode_window(keycode_rs: &str) -> Option<u16> {
    let mut used_hi = Vec::new();
    for line in keycode_rs.lines() {
        let Some(rest) = line.split("u16 = 0x").nth(1) else {
            continue;
        };
        let hex: String = rest.chars().take_while(char::is_ascii_hexdigit).collect();
        if let Ok(value) = u16::from_str_radix(&hex, 16) {
            used_hi.push((value >> 8) as u8);
        }
    }
    (0x78u8..=0xBF).find(|hi| !used_hi.contains(hi)).map(|hi| (hi as u16) << 8)
}

/// The substring strictly between `start` and the first `end` after it.
fn between<'a>(text: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let from = text.find(start)? + start.len();
    let len = text[from..].find(end)?;
    Some(&text[from..from + len])
}

/// Whether `s` is a non-empty ASCII identifier (no spaces/operators), to reject doc-comment
/// or attribute lines that happen to contain ` = `.
fn is_ident(s: &str) -> bool {
    let s = s.trim();
    !s.is_empty()
        && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
        && s.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
}
