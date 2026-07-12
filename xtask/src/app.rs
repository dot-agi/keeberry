// SPDX-License-Identifier: GPL-2.0-or-later
//! App-side wiring: keep the TypeScript client a faithful mirror of the firmware.
//!
//! Every kind adds the feature's row to the firmware fixture's `FEATURE_DEFS` (so the simulated
//! `GET_FEATURES` enumeration matches the registry). A `config` feature additionally gets the
//! full kcp mirror the drift-check pairs against — `Group`/`Cmd` in `protocol.ts`, the
//! `info.ts` group map, the fixture's `CAPABILITIES`/`SCHEMA_VERSION`/dispatch — plus a stamped
//! wire codec, a drift test, and a self-describing `FeatureDescriptor` (no hand-built React).

use std::path::Path;
use std::process::Command;

use crate::edit::{write_new, Doc};
use crate::names::Names;
use crate::resources::Resources;
use crate::templates::{self, APP_CODEC, APP_DESCRIPTOR, APP_TEST};
use crate::Kind;

/// Mirror the new feature into the TS client for `kind`.
pub fn wire(root: &Path, n: &Names, r: &Resources, kind: Kind) -> Result<(), String> {
    add_feature_def_row(root, n, r)?;
    if kind == Kind::Config {
        wire_protocol(root, n, r)?;
        wire_info(root, n, r)?;
        wire_fixture(root, n, r)?;
        wire_index(root, n)?;
        wire_persisted_write_cmds(root, n)?;
        stamp_codec_and_test(root, n, r)?;
        stamp_descriptor(root, n, r)?;
    }
    format_generated(root, n, kind);
    Ok(())
}

/// Run the app's own Prettier over the files this wiring stamped or edited, so the result is
/// Prettier-clean for *any* feature name. A long name overflows Prettier's print width on lines
/// the templates can't pre-wrap (e.g. the codec import, which Prettier collapses to one line for a
/// short name and wraps for a long one), so emitting fixed layout can't satisfy it across all
/// names — formatting the output once does. Best effort: a checkout that has not run `npm install`
/// (no `app/node_modules`) just gets a note pointing at `npm run format`, never a failed scaffold;
/// the build/test/drift gates remain the objective done-signal. `NODE_OPTIONS` is dropped from the
/// child so the formatter runs under default Node settings regardless of the caller's environment.
fn format_generated(root: &Path, n: &Names, kind: Kind) {
    let mut files = vec!["app/src/kcp/firmware-fixture.ts".to_string()];
    if kind == Kind::Config {
        files.extend([
            "app/src/kcp/protocol.ts".to_string(),
            "app/src/kcp/info.ts".to_string(),
            "app/src/kcp/index.ts".to_string(),
            "app/src/featureDescriptors/index.ts".to_string(),
            format!("app/src/kcp/{}.ts", n.snake),
            format!("app/src/kcp/{}.test.ts", n.snake),
            format!("app/src/featureDescriptors/{}.ts", n.snake),
        ]);
    }
    let prettier = root.join("app/node_modules/.bin/prettier");
    let hint = "format the generated files with `cd app && npm run format`";
    if !prettier.exists() {
        println!("\nnote: {} is absent — {hint}.", prettier.display());
        return;
    }
    match Command::new(&prettier)
        .arg("--write")
        .args(&files)
        .current_dir(root)
        .env_remove("NODE_OPTIONS")
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => println!("\nnote: prettier exited with {status} — {hint}."),
        Err(e) => println!("\nnote: could not run prettier ({e}) — {hint}."),
    }
}

/// The fixture's `FEATURE_DEFS` row — appended at the registry-END anchor, so repeated
/// generations keep the simulated enumeration in firmware `FEATURES` order (an `insert_after`
/// the last row would prepend each new feature, reversing the order vs the firmware). Done for
/// every kind so the simulated enumeration mirrors the firmware.
fn add_feature_def_row(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let mut doc = Doc::open(root.join("app/src/kcp/firmware-fixture.ts"))?;
    doc.insert_before(
        "// @scaffold:fixture-features",
        &format!("name: '{}'", n.display),
        &format!("  {{ id: {}, name: '{}', alwaysOn: false }},\n", r.feature_id, n.display),
    )?;
    doc.save()
}

/// `protocol.ts`: the `Group` nibble and the `Cmd.<Name>Get/Set` opcodes (the drift-check pairs
/// each against the firmware `group::`/`CMD_*`).
fn wire_protocol(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let nibble = format!("{:x}", r.nibble.ok_or("no free kcp nibble")?);
    let mut doc = Doc::open(root.join("app/src/kcp/protocol.ts"))?;
    doc.insert_before(
        "  Features: 0xd,",
        &format!("{}: 0x{nibble},", n.pascal),
        &format!("  {}: 0x{nibble},\n", n.pascal),
    )?;
    doc.insert_before(
        "  // --- FEATURES (0xDx) ---",
        &format!("{}Get:", n.pascal),
        &format!(
            "  // --- {NAME} (0x{nibble}x) ---\n  \
             /** {NAME} 0x{nibble}0 — get the {Display} config (CONFIG_LEN bytes). TODO(behavior): document. */\n  \
             {Name}Get: 0x{nibble}0,\n  \
             /** {NAME} 0x{nibble}1 — set one config field `[field, value]`. TODO(behavior): document. */\n  \
             {Name}Set: 0x{nibble}1,\n\n",
            NAME = n.screaming,
            Name = n.pascal,
            Display = n.display,
        ),
    )?;
    doc.save()
}

/// `info.ts`: the `GroupName` member and the `GROUP_DEFS` row, so the new group is *known* (it
/// renders and is never reported as an `unknownBit`).
fn wire_info(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let _ = r;
    let mut doc = Doc::open(root.join("app/src/kcp/info.ts"))?;
    doc.insert_before(
        "  | 'features'",
        &format!("| '{}'", n.camel),
        &format!("  | '{}'\n", n.camel),
    )?;
    doc.insert_before(
        "  { name: 'features', bit: Group.Features, label: 'Features' },",
        &format!("name: '{}'", n.camel),
        &format!("  {{ name: '{}', bit: Group.{}, label: '{}' }},\n", n.camel, n.pascal, n.display),
    )?;
    doc.save()
}

/// `firmware-fixture.ts`: bump `SCHEMA_VERSION`, OR the new bit into `CAPABILITIES`, add the
/// `FakeDevice` state + its seed, and a `<camel>Dispatch` wired into `fakeFirmwareHandle` — so a
/// generated drift test can round-trip GET/SET through the simulator with no hardware.
fn wire_fixture(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    let mut doc = Doc::open(root.join("app/src/kcp/firmware-fixture.ts"))?;

    doc.replace_once(
        &format!("export const SCHEMA_VERSION = {};", r.schema_version),
        &format!("export const SCHEMA_VERSION = {};", r.schema_version + 1),
    )?;
    // Keep the two hand-written doc-comments that quote the live literals in lockstep with the
    // bumped constants, so a generated fixture never contradicts its own prose (the SCHEMA_VERSION
    // doc-block and the CAPABILITIES `equals 0x…` note). Both literal spellings are unique to their
    // comment (the code uses different forms), so `replace_once` lands only on the prose.
    doc.replace_once(
        &format!("config::SCHEMA_VERSION: u16 = {}", r.schema_version),
        &format!("config::SCHEMA_VERSION: u16 = {}", r.schema_version + 1),
    )?;
    doc.replace_once(
        &format!("equals 0x{:04X}.", r.caps_old),
        &format!("equals 0x{:04X}.", r.caps_new),
    )?;
    doc.insert_before(
        "    (1 << Group.Features) |",
        &format!("(1 << Group.{})", n.pascal),
        &format!("    (1 << Group.{}) |\n", n.pascal),
    )?;
    doc.insert_after(
        "  unicode: UnicodeStateSample;",
        &format!("{}: {{ config: number[]", n.camel),
        &format!(
            "  /** {Display} config bytes (features/{name}.rs RAM): GET returns them, SET writes [field, value]. */\n  \
             {camel}: {{ config: number[] }};\n",
            Display = n.display,
            name = n.snake,
            camel = n.camel,
        ),
    )?;
    doc.insert_after(
        "    unicode: { mode: DEFAULT_UNICODE_STATE.mode, map: [...DEFAULT_UNICODE_STATE.map] },",
        &format!("{}: {{ config: new Array", n.camel),
        &format!(
            "    {}: {{ config: new Array<number>({}).fill(0) }},\n",
            n.camel,
            templates::CONFIG_LEN
        ),
    )?;
    doc.insert_before(
        "/** Mirror of `features::features_dispatch`",
        &format!("function {}Dispatch", n.camel),
        &format!(
            "/** Mirror of `features::{name}::on_kcp`: GET returns the config bytes, SET writes [field, value]. */\n\
             function {camel}Dispatch(cmd: number, req: Uint8Array, d: FakeDevice): [Status, Uint8Array] {{\n  \
               switch (cmd) {{\n    \
                 case Cmd.{Name}Get: {{\n      \
                   const out = replyPayload();\n      \
                   out.set(d.{camel}.config, 0);\n      \
                   return [Status.Ok, out];\n    \
                 }}\n    \
                 case Cmd.{Name}Set: {{\n      \
                   const field = req[2];\n      \
                   const value = req[3];\n      \
                   if (field >= d.{camel}.config.length) return [Status.BadArg, replyPayload()];\n      \
                   d.{camel}.config[field] = value;\n      \
                   return [Status.Ok, replyPayload()];\n    \
                 }}\n    \
                 default:\n      \
                   return [Status.BadCmd, replyPayload()];\n  \
               }}\n\
             }}\n\n",
            name = n.snake,
            camel = n.camel,
            Name = n.pascal,
        ),
    )?;
    doc.insert_before(
        "    default:\n      status = Status.Unsupported;",
        &format!("case Group.{}:", n.pascal),
        &format!(
            "    case Group.{Name}:\n      [status, payload] = {camel}Dispatch(cmd, req, device);\n      break;\n",
            Name = n.pascal,
            camel = n.camel,
        ),
    )?;
    doc.save()
}

/// `kcp/index.ts`: re-export the new wire codec.
fn wire_index(root: &Path, n: &Names) -> Result<(), String> {
    let mut doc = Doc::open(root.join("app/src/kcp/index.ts"))?;
    doc.insert_after(
        "export * from './features';",
        &format!("from './{}'", n.snake),
        &format!("export * from './{}';\n", n.snake),
    )?;
    doc.save()
}

/// `kcp/client.ts`: register the feature's `Cmd.<Name>Set` opcode in `PERSISTED_WRITE_CMDS`, so a
/// SET through the group — the descriptor's `runOp(Cmd.<Name>Set)` or a typed wrapper — marks the
/// session dirty and arms Save. Without this the generated config panel writes live to device RAM
/// with no unsaved-change signal, and the change is silently lost on the next reboot.
fn wire_persisted_write_cmds(root: &Path, n: &Names) -> Result<(), String> {
    let mut doc = Doc::open(root.join("app/src/kcp/client.ts"))?;
    doc.insert_before(
        "// @scaffold:persisted-write-cmds",
        &format!("  Cmd.{}Set,", n.pascal),
        &format!("  Cmd.{}Set,\n", n.pascal),
    )?;
    doc.save()
}

/// Stamp the TS wire codec and its drift test (`kcp/<name>.ts` + `kcp/<name>.test.ts`).
fn stamp_codec_and_test(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    write_new(
        root.join(format!("app/src/kcp/{}.ts", n.snake)),
        &templates::render(APP_CODEC, n, r),
    )?;
    write_new(
        root.join(format!("app/src/kcp/{}.test.ts", n.snake)),
        &templates::render(APP_TEST, n, r),
    )
}

/// Stamp the `FeatureDescriptor` (`featureDescriptors/<name>.ts`) and register it.
fn stamp_descriptor(root: &Path, n: &Names, r: &Resources) -> Result<(), String> {
    write_new(
        root.join(format!("app/src/featureDescriptors/{}.ts", n.snake)),
        &templates::render(APP_DESCRIPTOR, n, r),
    )?;
    let mut doc = Doc::open(root.join("app/src/featureDescriptors/index.ts"))?;
    doc.insert_before(
        "// @scaffold:descriptor-imports",
        &format!("import {{ {}Descriptor }} from './{}'", n.camel, n.snake),
        &format!("import {{ {0}Descriptor }} from './{1}';\n", n.camel, n.snake),
    )?;
    doc.insert_before(
        "// @scaffold:descriptor-registry",
        &format!("  {}Descriptor,", n.camel),
        &format!("  {}Descriptor,\n", n.camel),
    )?;
    doc.save()
}
