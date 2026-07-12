// SPDX-License-Identifier: GPL-2.0-or-later
//! `cargo xtask` — keeberry project automation.
//!
//! Today it exposes one command, the feature scaffolder behind `just new-feature <Name>`
//! (`.planning/sdk-llm-friendly.md` §4): one name + a `--kind` stamps every file, allocates the
//! next-free ids from the firmware's own resource tables, and threads the feature through every
//! `// @scaffold:` anchor on both the firmware and the app side — collapsing the ~17-edit,
//! ~12-file "scatter + allocation + mirror" of a new feature to *fill the marked `TODO(behavior)`*.
//! The build-time collision asserts and the protocol drift-check then prove the result; the
//! scaffolder never reasons about consistency, it just leaves a green validation loop to run.

mod app;
mod edit;
mod firmware;
mod names;
mod resources;
mod templates;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use names::Names;
use resources::Resources;

/// The shape of feature to scaffold — it selects the feature-file template and how much wiring
/// the feature needs (a `config` feature owns a kcp group and persists; the others do not).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A runtime-toggleable behaviour, rendered in the Features panel for free (no kcp/config).
    Toggle,
    /// A behaviour with a kcp config surface (its own group, opcodes, persistence, descriptor).
    Config,
    /// A behaviour fired from a dedicated keycode's press edge.
    Keycode,
}

impl Kind {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "toggle" => Ok(Self::Toggle),
            "config" => Ok(Self::Config),
            "keycode" => Ok(Self::Keycode),
            other => Err(format!("unknown --kind `{other}` (expected toggle | config | keycode)")),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Toggle => "toggle",
            Self::Config => "config",
            Self::Keycode => "keycode",
        }
    }
}

fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    match args.next().as_deref() {
        Some("new-feature") => new_feature(args.collect()),
        Some(other) => Err(format!("unknown command `{other}` (expected `new-feature`)")),
        None => Err("usage: cargo xtask new-feature <Name> [--kind toggle|config|keycode]".into()),
    }
}

/// `new-feature <Name> --kind <kind>`: allocate, stamp, wire, and print the checklist.
fn new_feature(args: Vec<String>) -> Result<(), String> {
    let (name, kind) = parse_new_feature_args(args)?;
    let names = Names::from_pascal(&name)?;
    let root = repo_root()?;

    let mod_rs = read(&root, "firmware/src/features/mod.rs")?;
    let kcp_rs = read(&root, "firmware/src/kcp.rs")?;
    let config_rs = read(&root, "firmware/src/config.rs")?;
    let keycode_rs = read(&root, "firmware/src/keycode.rs")?;
    let res = Resources::allocate(&mod_rs, &kcp_rs, &config_rs, &keycode_rs)?;

    if kind == Kind::Config && res.nibble.is_none() {
        return Err("no free kcp group nibble (0xB/0xC/0xE all taken) for a --kind config feature".into());
    }

    firmware::wire(&root, &names, &res, kind)?;
    app::wire(&root, &names, &res, kind)?;

    print_checklist(&names, &res, kind);
    Ok(())
}

/// Parse `<Name> [--kind <kind>]` (order-independent for `--kind`), defaulting to `toggle`.
fn parse_new_feature_args(args: Vec<String>) -> Result<(String, Kind), String> {
    let mut name = None;
    let mut kind = Kind::Toggle;
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--kind" => {
                let v = it.next().ok_or("--kind needs a value (toggle|config|keycode)")?;
                kind = Kind::parse(&v)?;
            }
            flag if flag.starts_with("--kind=") => kind = Kind::parse(&flag["--kind=".len()..])?,
            flag if flag.starts_with('-') => return Err(format!("unknown flag `{flag}`")),
            positional if name.is_none() => name = Some(positional.to_string()),
            extra => return Err(format!("unexpected argument `{extra}`")),
        }
    }
    let name = name.ok_or("a feature name is required, e.g. `new-feature DemoGizmo --kind config`")?;
    Ok((name, kind))
}

/// The monorepo root — the xtask crate's parent — verified to hold `firmware/` and `app/`.
fn repo_root() -> Result<PathBuf, String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or("xtask has no parent directory")?
        .to_path_buf();
    if !root.join("firmware").is_dir() || !root.join("app").is_dir() {
        return Err(format!("{} is not the keeberry root (no firmware/ + app/)", root.display()));
    }
    Ok(root)
}

fn read(root: &Path, rel: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(rel)).map_err(|e| format!("read {rel}: {e}"))
}

/// Print the only decisions left to a human/LLM — priority, hooks, field semantics — plus the
/// one-line validate command. The build-time asserts and the drift-check are the objective
/// "done" signal, so the checklist points at *running* them, never at asserting success.
fn print_checklist(n: &Names, r: &Resources, kind: Kind) {
    println!("\nScaffolded `{}` (--kind {}) — allocated:", n.pascal, kind.label());
    println!("  • FeatureId::{} = {}  (priority: appended last in FEATURES)", n.pascal, r.feature_id);
    if kind == Kind::Config {
        if let Some(nb) = r.nibble {
            println!("  • kcp group 0x{nb:X}  (CMD_{0}_GET 0x{nb:X}0 / CMD_{0}_SET 0x{nb:X}1)", n.screaming);
        }
        println!("  • config region before CRC_OFF, SCHEMA_VERSION -> {}", r.schema_version + 1);
    }
    if kind == Kind::Keycode {
        match r.keycode_window {
            Some(w) => println!("  • suggested keycode window 0x{w:04X}..=0x{:04X}", w | 0xFF),
            None => println!("  • keycode window: none free in 0x78xx..0xBFxx — pick one by hand"),
        }
    }

    println!("\nFill the TODO(behavior) markers, then decide:");
    println!("  1. Priority — move the `&{}::{}` line in features/mod.rs FEATURES to its correct", n.snake, n.screaming);
    println!("     dispatch position if it must run before another feature (it is last by default).");
    println!("  2. Hooks — implement the behaviour hook(s) in firmware/src/features/{}.rs", n.snake);
    match kind {
        Kind::Config => {
            println!("     (on_kcp GET/SET + snapshot_config/restore_config are stubbed; add the effect hook).");
            println!("  3. Fields — name the config bytes + ranges in features/{}.rs, the codec in", n.snake);
            println!("     app/src/kcp/{}.ts, and the controls in app/src/featureDescriptors/{}.ts.", n.snake, n.snake);
            println!("  4. Protocol-surface snapshots — a NEW kcp group + the SCHEMA bump change");
            println!("     hard-coded literals the hand-written tests assert. Update:");
            println!("       app/src/kcp/info.test.ts:");
            println!("         - the two CAPABILITIES `0x{:04x}` assertions -> 0x{:04x}", r.caps_old, r.caps_new);
            println!("         - add '{}' to `expectedPresent` (before 'features')", n.camel);
            println!("         - the schemaVersion literal {} -> {}", r.schema_version, r.schema_version + 1);
            if let (Some(nb), Some(next)) = (r.nibble, r.next_free_nibble) {
                println!("         - re-point the 'unknown capability bit' example off the now-claimed");
                println!("           bit 0x{nb:X} to the still-free 0x{next:X}.");
                println!("       app/src/kcp/codec.test.ts:");
                println!("         - the 'unknown group' example uses nibble 0x{nb:X} (now claimed) —");
                println!("           re-point it to the still-free 0x{next:X}.");
            }
        }
        Kind::Keycode => {
            println!("     and wire the keycode (no @scaffold anchor — by hand):");
            println!("  3. Keycode — in firmware/src/keycode.rs carve the window + a KeyAction variant +");
            println!("     its decode (with a disjointness assert); in firmware/src/keymap.rs call the");
            println!("     feature's press-edge method from compute_report, gated on features::is_enabled.");
        }
        Kind::Toggle => {
            println!("     (it already appears as a switch in the Features panel — no GUI code needed).");
        }
    }

    println!("\nValidate (the asserts + drift-check are the objective done-signal):");
    println!("  (cd firmware && cargo build -p keeberry --release --target thumbv7m-none-eabi --features {})", n.snake);
    println!("  (cd firmware && cargo clippy -p keeberry --release --target thumbv7m-none-eabi --features {} -- -D warnings)", n.snake);
    println!("  (cd app && npm run format && npm test && npm run check:protocol && npm run build && npm run lint)");
}
