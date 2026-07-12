// SPDX-License-Identifier: GPL-2.0-or-later
//! Firmware-side wiring: stamp `features/<name>.rs` and thread the feature through every
//! firmware site at its `// @scaffold:` anchor.
//!
//! The common wiring (module, `FeatureId`, the `< 32` guard, the `FEATURES` entry, the Cargo
//! feature, the config save/load arm) is done for every kind; a `config` feature also gets its
//! kcp group const + capability bit + dispatch arm + `CMD_*` opcodes and its persisted config
//! region + (de)serialise functions. Each edit is at an anchor or an exact line, so the diff is
//! deterministic and reviewable, and the build-time asserts then prove the allocation is sound.

use std::path::Path;

use crate::edit::{write_new, Doc};
use crate::names::Names;
use crate::resources::Resources;
use crate::templates::{self, FEATURE_CONFIG, FEATURE_KEYCODE, FEATURE_TOGGLE};
use crate::Kind;

/// Stamp the feature file and wire it into the firmware for `kind`.
pub fn wire(root: &Path, n: &Names, r: &Resources, kind: Kind) -> Result<(), String> {
    stamp_feature_file(root, n, r, kind)?;
    wire_registry(root, n, r)?;
    wire_cargo(root, n)?;
    wire_config_save_load(root, n, kind)?;
    if kind == Kind::Config {
        wire_kcp(root, n, r)?;
        wire_config_region(root, n, r)?;
    }
    Ok(())
}

/// Stamp `firmware/src/features/<name>.rs` from the `--kind` template.
fn stamp_feature_file(root: &Path, n: &Names, r: &Resources, kind: Kind) -> Result<(), String> {
    let template = match kind {
        Kind::Toggle => FEATURE_TOGGLE,
        Kind::Config => FEATURE_CONFIG,
        Kind::Keycode => FEATURE_KEYCODE,
    };
    let path = root.join(format!("firmware/src/features/{}.rs", n.snake));
    write_new(path, &templates::render(template, n, r))
}

/// `features/mod.rs`: the `pub mod`, the `FeatureId` variant, the `< 32` guard bump, and the
/// `FEATURES` registry entry (appended at the priority anchor — last, until repositioned).
fn wire_registry(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let mut doc = Doc::open(root.join("firmware/src/features/mod.rs"))?;
    doc.insert_before(
        "// @scaffold:features-mod",
        &format!("pub mod {};", n.snake),
        &format!("#[cfg(feature = \"{0}\")]\npub mod {0};\n", n.snake),
    )?;
    doc.insert_before(
        "// @scaffold:feature-id",
        &format!("{} = {},", n.pascal, r.feature_id),
        &format!(
            "    /// {} (the `{}` feature).\n    {} = {},\n",
            n.display, n.snake, n.pascal, r.feature_id
        ),
    )?;
    doc.replace_once(
        &format!("(FeatureId::{} as u32) < 32", r.prev_highest),
        &format!("(FeatureId::{} as u32) < 32", n.pascal),
    )?;
    doc.insert_before(
        "// @scaffold:features-registry",
        &format!("&{}::{},", n.snake, n.screaming),
        &format!(
            "    #[cfg(feature = \"{}\")]\n    &{}::{},\n",
            n.snake, n.snake, n.screaming
        ),
    )?;
    doc.save()
}

/// `firmware/Cargo.toml`: the `<name> = []` gate, and `"<name>"` added to `default`.
fn wire_cargo(root: &Path, n: &Names) -> Result<(), String> {
    let mut doc = Doc::open(root.join("firmware/Cargo.toml"))?;
    doc.insert_before(
        "# @scaffold:cargo-feature",
        &format!("\n{} = []", n.snake),
        &format!("{} = []\n", n.snake),
    )?;
    doc.append_default_feature(&n.snake)?;
    doc.save()
}

/// `config.rs`: the `save_feature` / `load_feature` arms. A `config` feature (de)serialises its
/// region; every other kind persists nothing, so it gets a no-op arm (which still keeps the
/// `match` exhaustive over the new, un-gated `FeatureId` variant).
fn wire_config_save_load(root: &Path, n: &Names, kind: Kind) -> Result<(), String> {
    let mut doc = Doc::open(root.join("firmware/src/config.rs"))?;
    let (save_arm, load_arm) = if kind == Kind::Config {
        (
            format!("        FeatureId::{} => serialize_{}(buf),\n", n.pascal, n.snake),
            format!("        FeatureId::{} => deserialize_{}(buf),\n", n.pascal, n.snake),
        )
    } else {
        let arm = format!("        FeatureId::{} => {{}}\n", n.pascal);
        (arm.clone(), arm)
    };
    doc.insert_after_scoped(
        "pub(crate) fn save_feature(id: FeatureId, buf: &mut [u8]) {",
        "match id {",
        &save_arm,
    )?;
    doc.insert_after_scoped(
        "pub(crate) fn load_feature(id: FeatureId, buf: &[u8]) {",
        "match id {",
        &load_arm,
    )?;
    doc.save()
}

/// `kcp.rs` (config only): the group const, the cfg-gated capability bit, the dispatch arm, and
/// the `CMD_<NAME>_GET`/`SET` opcode section.
fn wire_kcp(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let nibble = format!("{:x}", r.nibble.ok_or("no free kcp nibble for a config feature")?);
    let mut doc = Doc::open(root.join("firmware/src/kcp.rs"))?;

    doc.insert_before(
        "// @scaffold:kcp-group",
        &format!("pub const {}: u8", n.screaming),
        &format!(
            "    /// `0x{nibble}x` — {}: TODO(behavior) describe the group. Owned by the `{}`\n    \
             /// feature (routed through `features::run_on_kcp`); gated on its Cargo feature.\n    \
             #[cfg(feature = \"{}\")]\n    pub const {}: u8 = 0x{nibble};\n",
            n.display, n.snake, n.snake, n.screaming
        ),
    )?;

    doc.insert_before(
        "const CAPABILITIES: u32 =",
        &format!("const {}_CAP: u32", n.screaming),
        &format!(
            "/// The {0} capability bit, gated on the `{1}` feature that owns group `0x{nibble}`:\n\
             /// `1 << group::{0}` when built in, else `0`. Paired with the cfg-gated dispatch arm so a\n\
             /// build without the feature neither advertises the bit nor answers the group.\n\
             #[cfg(feature = \"{1}\")]\n\
             const {0}_CAP: u32 = 1 << group::{0};\n\
             #[cfg(not(feature = \"{1}\"))]\n\
             const {0}_CAP: u32 = 0;\n\n",
            n.screaming, n.snake
        ),
    )?;
    doc.insert_before(
        "    | (1 << group::SYSTEM);",
        &format!("| {}_CAP", n.screaming),
        &format!("    | {}_CAP\n", n.screaming),
    )?;

    // The anchor literal `// @scaffold:kcp-dispatch` also appears in the kcp-group anchor's
    // prose ("...dispatch arm at `// @scaffold:kcp-dispatch`."), so match the real site by its
    // trailing em-dash to land the arm in the dispatch `match`, not the `mod group` comment.
    doc.insert_before(
        "// @scaffold:kcp-dispatch \u{2014}",
        &format!("group::{} =>", n.screaming),
        &format!(
            "        #[cfg(feature = \"{}\")]\n        group::{} => features::run_on_kcp(cmd, req_payload, out),\n",
            n.snake, n.screaming
        ),
    )?;

    doc.insert_before(
        "// === FEATURES group (0xDx) ===",
        &format!("CMD_{}_GET", n.screaming),
        &format!(
            "// === {NAME} group (0x{nibble}x) ==================================================\n\
             //\n\
             // {Display}, owned by the `{name}` registry feature (routed through `features::run_on_kcp`).\n\
             // Opcodes declared here beside every other group's, as the single source the TS client and\n\
             // the drift check mirror; gated on the Cargo feature like the group const and dispatch arm.\n\n\
             /// {NAME} `0x{nibble}0` — get the {Display} config. No request payload; reply `[field0, …]`\n\
             /// (CONFIG_LEN bytes). TODO(behavior): document the real reply layout.\n\
             #[cfg(feature = \"{name}\")]\n\
             pub const CMD_{NAME}_GET: u8 = 0x{nibble}0;\n\
             /// {NAME} `0x{nibble}1` — set one config field. Request `[field, value]`; an out-of-range\n\
             /// field answers [`Status::BadArg`]. TODO(behavior): document the real request layout.\n\
             #[cfg(feature = \"{name}\")]\n\
             pub const CMD_{NAME}_SET: u8 = 0x{nibble}1;\n\n",
            NAME = n.screaming,
            name = n.snake,
            Display = n.display,
        ),
    )?;
    doc.save()
}

/// `config.rs` (config only): bump `SCHEMA_VERSION`, chain a fixed config region in before
/// `CRC_OFF`, and add the `serialize_<name>` / `deserialize_<name>` functions.
fn wire_config_region(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let mut doc = Doc::open(root.join("firmware/src/config.rs"))?;

    doc.replace_once(
        &format!("pub const SCHEMA_VERSION: u16 = {};", r.schema_version),
        &format!("pub const SCHEMA_VERSION: u16 = {};", r.schema_version + 1),
    )?;

    // The new region takes the offset CRC_OFF currently holds; CRC_OFF re-chains past it. Reading
    // the live expression means a second config feature chains cleanly onto the first.
    doc.insert_before(
        "/// Offset of the trailing CRC-32 word (it covers every byte before it).",
        &format!("const {}_OFF", n.screaming),
        &format!(
            "/// Offset of the {Display} config region (schema v{ver}). TODO(behavior): document the fields.\n\
             const {NAME}_OFF: usize = {expr};\n\
             /// Bytes the {Display} config region occupies (mirror of `features::{name}::CONFIG_LEN`).\n\
             const {NAME}_BYTES: usize = {len};\n\n",
            Display = n.display,
            ver = r.schema_version + 1,
            NAME = n.screaming,
            name = n.snake,
            expr = r.crc_expr,
            len = templates::CONFIG_LEN,
        ),
    )?;
    doc.replace_once(
        &format!("const CRC_OFF: usize = {};", r.crc_expr),
        &format!("const CRC_OFF: usize = {0}_OFF + {0}_BYTES;", n.screaming),
    )?;

    doc.insert_before(
        "/// Snapshot the feature identified by `id`",
        &format!("fn serialize_{}(", n.snake),
        &format!(
            "/// Snapshot the {Display} config region — the persist half of `{name}`'s `on_save`.\n\
             #[cfg(feature = \"{name}\")]\n\
             fn serialize_{name}(buf: &mut [u8]) {{\n    \
                 let region = &mut buf[{NAME}_OFF..{NAME}_OFF + {NAME}_BYTES];\n    \
                 crate::features::{name}::{NAME}.snapshot_config(region);\n\
             }}\n\
             /// A no-op when `{name}` is absent: the blob still reserves the region, so the offsets and\n\
             /// `BLOB_LEN` are a stable layout independent of which features are built in.\n\
             #[cfg(not(feature = \"{name}\"))]\n\
             fn serialize_{name}(_buf: &mut [u8]) {{}}\n\n\
             /// Restore the {Display} config region — the restore half of `{name}`'s `on_load`.\n\
             #[cfg(feature = \"{name}\")]\n\
             fn deserialize_{name}(buf: &[u8]) {{\n    \
                 let region = &buf[{NAME}_OFF..{NAME}_OFF + {NAME}_BYTES];\n    \
                 crate::features::{name}::{NAME}.restore_config(region);\n\
             }}\n\
             #[cfg(not(feature = \"{name}\"))]\n\
             fn deserialize_{name}(_buf: &[u8]) {{}}\n\n",
            Display = n.display,
            name = n.snake,
            NAME = n.screaming,
        ),
    )?;
    doc.save()
}
