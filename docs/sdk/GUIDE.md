<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# keeberry feature SDK — contributor guide

How to add a new firmware feature **and** its configurator surface to keeberry,
and the mental model that makes the path short. Read this once; thereafter the
[REFERENCE](./REFERENCE.md) is the lookup and [EXAMPLE](./EXAMPLE.md) is the
copy-paste pattern. An agent picks the same path up automatically through the
`keeberry-feature` skill.

## What a feature is (and isn't)

A keeberry feature is a **compile-time plugin**: one `impl Feature` added to an
ordered registry, compiled into the firmware image. This is the same shape as a
[QMK Community Module](https://docs.qmk.fm/features/community_modules) or a ZMK
behaviour — and, like both, it is **100% compile-time**. keeberry runs no_std on
a WB32FQ95 with 128 KB flash and cable-only DFU; there is no runtime plugin
loader and there cannot be one. Shipping a feature means a firmware build and a
reflash.

"Easy to contribute" therefore does not mean runtime-loadable. It means the
authoring path is **short, local, declarative, and self-verifying**:

- **Local** — one feature is one Rust file (plus, for a config feature, one
  descriptor file). You do not hold the whole codebase in your head.
- **Declarative** — the GUI is data (a descriptor), not hand-written React; the
  registry is an explicit array you can read top to bottom.
- **Self-verifying** — a resource collision is a *build error* and a firmware↔app
  mismatch is a *failing drift check*. You never reason about consistency; you
  make the validation loop green.

The runtime configurator still surfaces every feature live (enable/disable,
parameters) — but the *code* ships at build time.

## The mental model

### The `Feature` trait and its hooks

`firmware/src/features/mod.rs` defines one trait, `Feature`, with a hook for each
seam in the keyboard loop. Every hook has a no-op default, so a feature
**overrides only what it uses** — a toggle feature might implement one hook; a
rich feature several.

The hooks fall into three dispatch styles, which is the whole behavioural
contract:

- **fold** (`on_matrix`, `on_report`, `on_overlay`) — every enabled, active
  feature transforms the value in turn, **in array order**. SOCD cleans the key
  set, then overrides rewrite it, then caps-word shifts it, and so on.
- **first-claims** (`on_kcp`, `on_rgb_frame`) — the first feature to claim the
  request (return `Some`/`true`) handles it; the rest are skipped.
- **sequence** (`on_tick`, `on_save`, `on_load`) — every feature runs, for
  periodic work and persistence.

See [REFERENCE → The Feature trait](./REFERENCE.md#the-feature-trait) for each
hook's signature, when it fires, and what to put in it.

### The registry — `static FEATURES`, array order = priority

All features live in one ordered array, `static FEATURES: &[&dyn Feature]`. This
is a deliberate design choice over link-time auto-registration (`linkme`/
`inventory`): an explicit array can be read top to bottom and grepped, and — the
decisive reason — **array order is dispatch priority**, a load-bearing invariant
the linker cannot express. The order encodes real rules (SOCD before overrides;
autocorrect last so its overlay has the final say). A `#[cfg(feature = "…")]`
gate on an entry means a disabled Cargo feature drops the module, the registry
entry, and all its flash.

You never edit this array by hand — the scaffolder writes the one line — but you
should understand it, because **where** your entry sits decides when your fold
hook runs relative to the others.

### The central enable bitmap

Every feature has a persisted master on/off switch, held **centrally** in one
`AtomicU32` (`ENABLED`), one bit per `FeatureId`, not per feature. That single
word is why the GUI's Features panel needs zero per-feature code: the firmware
enumerates the registry over the FEATURES kcp group and the app renders one
toggle each. It is also why a *toggle* feature needs no `config.rs` edit — its
bit rides the existing central word that the config blob already persists.

The per-scan hooks skip a feature whose bit is clear (or whose `active()` is
false), so a disabled or idle feature is wholly inert at ~one atomic load per
scan — the 1 kHz budget is preserved. `on_disable()` clears a feature's transient
state on the off-edge so a re-enable starts clean. A structural feature
(`FEATURE_ALWAYS_ON`) cannot be switched off.

### Stable identity — `FeatureId`

`FeatureId` is a `#[repr(u8)]` enum; its discriminant is the feature's **stable
wire and persistence id** — the bit it occupies in `ENABLED`, and the id the
FEATURES group reports to the GUI. Keep discriminants contiguous from 0; a new
feature takes the next free value; never renumber a shipped one. (Config
save/load matches by variant name, not number, so the exact values do not affect
persistence — but the wire contract does.)

### The two-sided wire contract

Everything the firmware exposes to the app over kcp — group nibbles, opcodes,
payload layouts, `SCHEMA_VERSION` — is mirrored by hand in TypeScript
(`app/src/kcp/*`). The firmware is the **single source of truth**
(`protocol/README.md`); the mirror is kept honest by `npm run check:protocol`,
a CI-gated drift check that compares both sides constant-for-constant and
opcode-for-opcode. A config feature adds a small codec + a fixture entry on the
app side; the drift check proves they match the firmware. This is why an LLM can
verify a feature with no keyboard plugged in.

## The end-to-end path (what the scaffolder automates)

Adding a maximal feature by hand touches ~17 edits across ~12 files: a
`FeatureId` variant, the registry entry, a `Cargo.toml` feature, a bumped assert,
maybe a keycode window, maybe a kcp group + opcodes + dispatch, maybe a config
region + a `SCHEMA_VERSION` bump, then the whole app mirror (protocol consts,
codec, client method, fixture, drift test) and the GUI. Most of that is
**mechanical** (rote, mirror-able) or a **decision** (which free slot); only ~3
edits are real value (behaviour, param schema, any custom GUI).

`just new-feature <Name> --kind <toggle|config|keycode>` collapses it to **one
command + filling the marked TODOs**. It:

1. **Allocates** from the resource tables the build-time asserts already guard:
   the next-free `FeatureId` (and bumps the `< 32` guard), and for the kind, a
   kcp nibble / a keycode sub-window / a config region (+ the `SCHEMA_VERSION`
   bump).
2. **Stamps** `firmware/src/features/<name>.rs` from the kind's template, with
   `// TODO(behavior)` at every hook body and a doc-comment skeleton.
3. **Wires the firmware** — the `mod`, the `FeatureId` variant, the `FEATURES`
   entry, the `Cargo.toml` feature, and for `config` the kcp group const +
   capability bit + dispatch arm + `CMD_*` opcode stubs (+ the persistence
   region + serialize/deserialize stubs).
4. **Stamps and wires the app** — the `kcp/<name>.ts` codec, the `Group`/`Cmd`
   entries, the `index.ts` export, the `firmware-fixture` row + dispatch stub,
   the `kcp/<name>.test.ts` drift-test skeleton, and for `config` the
   `app/src/featureDescriptors/<name>.ts` descriptor.
5. **Prints the checklist** of what is left: the priority position (if it
   matters), which hooks to implement, and the field semantics.

Then you fill the behaviour and (for config) the schema, and run the validation
gate. The generated drift tests + the build-time asserts *prove* the mirror is
consistent.

> **The one hand-edit the scaffolder can't make.** Everything above is in-tree and
> real — the trait, registry, enable bitmap, kcp groups, and config blob in
> `firmware/src/`, the `feature!{}` macro in `firmware/src/features/macros.rs`, the
> scaffolder in `xtask/src/`, and the `FeatureDescriptor`/generic-panel layer in
> `app/src/`. The scaffolder stamps and wires all of it; it does **not** rewrite
> test assertions. A `--kind config` feature grows the protocol surface (a new
> `CAPABILITIES` bit + a `SCHEMA_VERSION` bump), and a few hand-written snapshot
> tests assert those as literals, so the printed checklist names ~3 by-hand edits in
> `app/src/kcp/info.test.ts` and `codec.test.ts` — the new caps mask, the bumped
> `schemaVersion`, the new group in `expectedPresent`, and re-pointing the "unknown
> bit/group" examples off your now-claimed nibble. That apart, you fill the behaviour
> and the schema and run the gate.

## The three kinds

The `--kind` selects the starting template. They are starting points, not silos:
a feature can be keycode-triggered *and* own config; pick the dominant shape and
add the other axis by hand (or run the scaffolder once per axis where supported).

### `toggle` — behaviour with a master switch

The minimal feature: state in an atomic (or a small `Mutex<RefCell<…>>`), an
`active()` gate, and the one or two hooks that implement the behaviour. No kcp
group, no persistence beyond the central enable bit, **no app code** — it appears
in the Features panel automatically. Canonical example: `caps_word.rs` (one
`AtomicBool`, one `on_report`). Use this kind for "Shift held for a word", "flash
an LED on layer change", "swap two keys while held" — anything whose only
configuration is on/off.

### `config` — host-set parameters

A feature with a panel of settings: it owns a kcp group (a free nibble), answers
`get`/`set` opcodes in `on_kcp` (model: `autocorrect.rs`), optionally persists
its params in a config region, and ships an `app/src/featureDescriptors/<name>.ts`
descriptor that the generic panel renders. Use this for "a brightness slider", "a
configurable timeout", "an effect dropdown" — anything with numbers or choices.
The descriptor is **data**, so the app PR is small and reviewable.

### `keycode` — a bindable keycode

A feature the user triggers by binding a dedicated keycode on the keymap. It
carves a sub-window in the `keycode.rs` 16-bit space (with a disjointness
assert), adds a `KeyAction` variant + its decode, and is invoked on the press
edge from `keymap::compute_report` — gated by `is_enabled` so a disabled feature
ignores its key. Model: `caps_word.rs` (the `CapsWord` keycode engages it). Use
this for "a key that types my email", "a key that cycles the active profile",
"a leader-style trigger".

## Where to go next

- The full API — every hook, the macro, the descriptor schema, the kcp and
  persistence conventions, and every build-time assert with its fix: see the
  [REFERENCE](./REFERENCE.md).
- A complete feature built from scratch, every file shown, ending in the green
  gate: see the [EXAMPLE](./EXAMPLE.md).
