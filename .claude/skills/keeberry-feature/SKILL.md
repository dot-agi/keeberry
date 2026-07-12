---
name: keeberry-feature
description: >-
  Add or modify a keeberry firmware feature and/or its configurator panel end to
  end. Use whenever the user wants a new keyboard behaviour, effect, input mode,
  lighting trick, or app config surface on the keeberry (WB32 Rust firmware +
  React/WebHID app) — phrasings like "add a feature", "make the keyboard do X",
  "a new Features-panel toggle", "expose a setting in the app", "a per-layer/
  per-key X", "flash an LED when Y", "a new keycode that Z", or changing an
  existing feature's behaviour or controls — even if they never say "feature",
  "plugin", "SDK", or "kcp". Covers the Feature trait + registry, the runtime
  enable bitmap, kcp groups/opcodes, config persistence, the app codec/fixture/
  drift tests, and the auto-rendered GUI. Not for one-off keymap edits in the
  app, MCU bring-up, or transport/protocol changes unrelated to a feature.
allowed-tools: Bash(cargo:*) Bash(npm:*) Bash(just:*) Read Edit Write
paths:
  - firmware/src/features/**
  - app/src/kcp/**
  - app/src/featureDescriptors/**
  - app/src/ui/**
---

# Adding a keeberry feature

A keeberry feature is a **compile-time plugin** — one `impl Feature` in the
ordered `FEATURES` registry, exactly like a QMK Community Module or a ZMK
behaviour. There is no runtime loading (no_std, 128 KB flash, cable-only DFU).
"Easy to add" means the authoring path is short, local, declarative, and
**self-verifying**: a resource collision is a build error and a firmware↔app
mismatch is a failing drift check, so you never reason about consistency — you
make the loop green.

Work the loop in order: **scaffold → fill → validate**. Do not hand-wire the
registry, allocate ids by hand, or claim "done" from inspection — the scaffolder
allocates and wires; the gate in step 4 is the only thing that says done.

## 1. Decide the shape

Pick the `--kind` whose template is closest; kinds combine (a keycode feature
can also own config). When unsure, read `docs/sdk/GUIDE.md` ("The three kinds").

| The feature is driven by… | `--kind` | Template | Touches |
|---|---|---|---|
| an on/off switch + behaviour hooks, no params | `toggle` | `caps_word.rs` | one Rust file; **zero app code** (auto-toggle), **zero config edit** |
| host-set parameters (a panel of controls) | `config` | `autocorrect.rs` | Rust file + a kcp group + an `app/src/featureDescriptors/<name>.ts` |
| a dedicated keycode the user binds on the keymap | `keycode` | `caps_word.rs` | Rust file + a `keycode.rs` window + a press-edge call |

A *toggle* feature already appears in the GUI for free: the firmware enumerates
the registry over the FEATURES kcp group and `app/src/ui/FeaturesPanel.tsx`
renders one switch per feature with no per-feature code. You only build app-side
UI for a *config* feature, and even then it is a declarative descriptor, not a
React panel.

## 2. Scaffold

```
just new-feature <Name> --kind <toggle|config|keycode>
```

This stamps the feature file(s), allocates the next-free `FeatureId` (and any
kcp nibble / keycode window / config region), bumps the matching build-time
assert, wires the registry + `Cargo.toml` + the app mirror (codec, fixture row,
drift-test skeleton, and — for `config` — the descriptor stub), and prints the
checklist of what is left to fill. **Never edit the registry or the resource
tables by hand** — that is the scatter the scaffolder exists to remove.

> The scaffolder allocates ids, stamps the files, and wires the registry +
> `Cargo.toml` + the app mirror; it does **not** rewrite test assertions. A
> `--kind config` feature grows the protocol surface (a new `CAPABILITIES` bit + a
> `SCHEMA_VERSION` bump), so its printed checklist names ~3 by-hand edits in
> `app/src/kcp/info.test.ts` and `codec.test.ts` — the new caps mask, the bumped
> `schemaVersion`, the new group in `expectedPresent`, and re-pointing the "unknown
> bit/group" examples off the now-claimed nibble. The checklist prints the exact
> old→new values; everything else is stamped.

## 3. Fill the TODOs

The scaffold leaves `// TODO(behavior)` markers at exactly two places:

1. **The behaviour** — the hook bodies in `firmware/src/features/<name>.rs`.
   Override only the hooks you use (every hook defaults to a no-op). Two hard
   rules, both load-bearing:
   - **No `RefCell` borrow held across `.await`.** Every hook is synchronous;
     keep it so. The cooperative executor must never interleave a reader and a
     writer of the same table mid-borrow.
   - **Honour the 1 kHz budget.** Gate idle work behind `active()` so a feature
     that is doing nothing costs one relaxed atomic load per scan, not real
     work. Keycode entry points must early-return when `!is_enabled(self.id())`
     (see `caps_word.rs::engage`).

   Which hook does what (fold / first-claims / sequence, and when each fires) is
   in `docs/sdk/REFERENCE.md` ("The Feature trait").

2. **The schema** — for `--kind config`, the controls in
   `app/src/featureDescriptors/<name>.ts`. Each control is data (`kind` +
   `label` + the kcp `get`/`set` op), rendered by one generic panel. The control
   kinds and the `showIf` conditional are in `docs/sdk/REFERENCE.md`
   ("FeatureDescriptor"). Mirror the firmware op layout the scaffolder stamped.

For a config feature, the firmware fills all live in
`firmware/src/features/<name>.rs` (model: `autocorrect.rs`): the `on_kcp` SET
validation, the `snapshot_config`/`restore_config` bodies, and the
`// TODO(behavior)` effect hook. The `config.rs` `serialize_*`/`deserialize_*`
(plus the region offset and `SCHEMA_VERSION` bump) are generated **complete** —
they just delegate to `snapshot_config`/`restore_config`, so you never edit
`config.rs`. A toggle feature's only persistent state is its enable bit, which
rides the central bitmap automatically — likewise no `config.rs` edit.

When in doubt, copy the fully-worked feature in `docs/sdk/EXAMPLE.md` — it shows
every generated file filled in, then the green gate.

## 4. Validate — the hard gate

This is the objective done-signal. Run every command and **read the output**;
do not claim success until all are green. The build-time asserts and the drift
check are what make a resource collision or a firmware↔app mismatch a hard
failure here instead of a silent runtime bug.

```
cd firmware && cargo build --features <name>   # collision asserts must pass
cargo clippy --features <name>                 # lint clean
cd ../app && npm run format                     # Prettier the generated + filled files
npm test                                        # feature unit + drift tests pass
npm run check:protocol                          # firmware↔app wire format in lockstep
npm run build                                   # app type-checks + bundles
npm run lint                                    # app lint clean
```

This is the full app loop the scaffolder prints. Iterate until every command is
green. Common failures and their fix are in `docs/sdk/REFERENCE.md` ("Build-time
asserts"); each assert message names the exact constant to bump. Before a PR, also
run `cargo build --release` to prove the firmware still links and fits flash.

## Pitfalls

- **A new persisted param bumps `SCHEMA_VERSION`, which invalidates every saved
  config.** Config validity is exact-match (magic + version + CRC), so a version
  bump resets users to defaults on next boot — there is no migration. Call it
  out in the PR. A *toggle* feature does **not** bump the schema (its bit is in
  the existing central enable word).
- **The kcp reply payload is 29 bytes.** A reply that does not fit must page
  (model: `CMD_GET_FEATURES`). The per-group `assert!(… >= …_LEN, "… must fit
  one reply")` catches an overrun at build time.
- **A fresh kcp nibble is scarce** — only `0xB`, `0xC`, `0xE` are free. Reuse
  the BEHAVIOR/TEXT sub-opcode space unless the feature is a genuinely new
  domain.
- **`name()` is the GUI label** and must fit one reply alongside its record;
  keep it short (≤ ~20 bytes).
- **Array order is priority.** If your feature's report fold must run before or
  after another's, place its `FEATURES` entry accordingly and say why in a
  comment (see the ordering rationale on `FEATURES`).
