<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# keeberry feature SDK — reference

The lookup for adding a feature: every `Feature` hook, the `feature!{}` macro,
the `FeatureDescriptor` schema, the kcp and persistence conventions, the four
resource tables, and every build-time assert with its fix. Read the
[GUIDE](./GUIDE.md) first for the mental model; copy the [EXAMPLE](./EXAMPLE.md)
for the end-to-end pattern.

Source of truth, all in-tree:
`firmware/src/features/mod.rs` (trait, registry, enable bitmap),
`firmware/src/kcp.rs` (framing, groups, Status, opcodes),
`firmware/src/config.rs` (persistence, `SCHEMA_VERSION`),
`firmware/src/keycode.rs` (keycode windows),
`app/src/kcp/*` (the TS mirror), `protocol/README.md` (the drift contract).

The whole path is in-tree and shipped: the `just new-feature` scaffolder (a
`cargo xtask`, sources in `xtask/src/`), the `feature!{}` macro
(`firmware/src/features/macros.rs`), the `FeatureDescriptor`/generic-panel layer
(`app/src/featureDescriptors/` + `app/src/ui/DescriptorPanel.tsx`), and the
build-time asserts + drift check that prove a feature consistent. Start from the
[scaffolder](#the-just-new-feature-scaffolder) — it stamps and wires everything
below; the rest of this reference is what it generates and how to fill it.

## The Feature trait

Defined in `firmware/src/features/mod.rs`. Every hook has a no-op default — a
feature overrides only what it uses. Each per-scan fold/sequence hook runs only
for a feature that is both **enabled** (its `ENABLED` bit set) and **active**
(`active()` true); `on_kcp`, `on_save`, and `on_load` are *not* gated (reasons
below).

### Identity and lifecycle

| Hook | Signature | When to use it |
|---|---|---|
| `id` | `fn id(&self) -> FeatureId` | **Required.** Return the feature's `FeatureId` variant. The discriminant is the stable wire + persistence id (the `ENABLED` bit, the id the GUI sees). |
| `name` | `fn name(&self) -> &'static str` | **Required.** The GUI label in the auto-rendered Features panel. A `&'static str` (ptr+len in flash); keep it short (≤ ~20 bytes) so its record fits one kcp reply. |
| `flags` | `fn flags(&self) -> u8` | Override to declare capabilities. Default = `FEATURE_DEFAULT_ON`. OR in `FEATURE_ALWAYS_ON` for a structural feature that must never be switched off. |
| `active` | `fn active(&self) -> bool` | The O(1) "is anything to do?" gate — typically one relaxed atomic load. Default `true`. Override it whenever the feature has an idle state, so the per-scan hooks skip it cheaply (the 1 kHz budget). |
| `on_disable` | `fn on_disable(&self)` | Clear transient state on the on→off edge (called by `set_enabled`), so a disable mid-action strands nothing and a re-enable starts clean. Default no-op; a stateless or always-on feature needs nothing here. |

`flags` bits (in `firmware/src/features/mod.rs`):

- `FEATURE_DEFAULT_ON = 1 << 0` — ships enabled; a reset or a config blob without
  the feature's bit falls back to this.
- `FEATURE_ALWAYS_ON = 1 << 1` — structural; `set_enabled` rejects a disable with
  `Status::BadArg` and the bit is force-held on through a config restore.

> `FEATURE_HAS_CONFIG` / `FEATURE_PERSISTS` appear in the design as future,
> conservative-default capability hints (a feature *declaring* its side-effects).
> They are **not in the code today** — only the two bits above exist. Do not
> reference them until they land.

### Behaviour hooks

| Hook | Signature | Dispatch | When to use it |
|---|---|---|---|
| `on_matrix` | `fn on_matrix(&self, c: &Ctx, m: &mut [u16; NUM_ROWS])` | fold | Transform the effective matrix before the report builder resolves it — e.g. suppress claimed key positions. |
| `on_report` | `fn on_report(&self, c: &Ctx, mods: &mut u8, keys: &mut KeySet)` | fold | Rewrite the resolved modifier byte and basic-key set as the report is built (SOCD, overrides, caps-word Shift). The most common behaviour hook. |
| `on_overlay` | `fn on_overlay(&self, c: &Ctx, r: &mut Report)` | fold | Merge synthesised output into the finished report — an injected tap-dance/combo/macro key, or a played-back correction. |
| `on_rgb_frame` | `fn on_rgb_frame(&self, c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) -> bool` | first-claims | Paint a status overlay over the whole rendered RGB frame; return `true` to claim the frame (vetoing the base effect). Base per-key effects are a separate registry (`rgb::RGB_EFFECTS`). |
| `on_kcp` | `fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status>` | first-claims | Answer this feature's kcp config group. `Some(status)` claims `cmd`; `None` passes it on. See [kcp conventions](#kcp-group-and-opcode-conventions). **Ungated** — a feature is configurable while disabled, so the GUI can set it up before switching it on. |
| `on_tick` | `fn on_tick(&self, now: Instant)` | sequence | Timeout-driven state with no matrix edge to ride (a one-shot that expires, a debounce window). |
| `on_save` | `fn on_save(&self, out: &mut [u8])` | sequence | Snapshot the feature's RAM state into the config blob at its fixed offset. **Ungated** (an idle feature still writes its deterministic region so the layout is fixed). See [persistence](#config-persistence). |
| `on_load` | `fn on_load(&self, buf: &[u8])` | sequence | Restore the feature's RAM state from the blob. **Ungated** (a feature must rebuild its table even though it is idle until it does). |

`Ctx` (the read-only per-scan context the fold hooks share) carries `now`
(scan timestamp), `active_layers` (bitmask), and `prev_matrix` (last scan's
debounced matrix — the edge basis).

**Dispatch semantics** (the dispatch functions in `mod.rs`):

- **fold** (`run_on_matrix`/`run_on_report`/`run_on_overlay`) — every enabled,
  active feature transforms the value in turn, in `FEATURES` array order. Order
  is priority; place your entry to run before/after the features you compose
  with.
- **first-claims** (`run_on_kcp`/`run_on_rgb_frame`) — the first feature to
  return `Some`/`true` wins; the rest are skipped.
- **sequence** (`run_on_tick`/`run_on_save`/`run_on_load`) — every feature runs.

### The two hard rules

1. **No `RefCell` borrow held across `.await`.** Every hook is synchronous and
   must stay so. The cooperative executor would otherwise interleave a reader and
   a writer of the same `Mutex<RefCell<…>>` table mid-borrow and panic. Take the
   borrow, do the work, drop it — never `.await` inside it.
2. **Honour the 1 kHz budget via `active()`.** The keyboard loop runs the per-scan
   hooks every millisecond. A feature that is idle must cost ~one relaxed atomic
   load, not real work — so gate idle state behind `active()`, and have any
   keycode entry point early-return on `!is_enabled(self.id())` (model:
   `caps_word.rs::engage`).

## The `feature!{}` macro

`firmware/src/features/macros.rs` defines `feature!`, the declarative front door
for a `Feature`. It fuses the three rote items every feature repeats — the state
**struct**, its `'static` **singleton** (the value `FEATURES` points at), and the
`id()`/`name()`/`flags()` impl — into one declaration, leaving only the state and
the behaviour hooks to write. It is in scope in every feature module via
`#[macro_use] mod macros;`; it is **not** `#[macro_export]`ed, so there is no
`crate::feature` path — write it `feature!`.

```rust
// one invocation per feature, in features/<name>.rs
feature! {
    /// Optional outer docs/attributes for the generated struct.
    Foo as FOO,                     // the feature struct, and its `static` singleton's name
    id    = FeatureId::Foo,         // the FeatureId discriminant (the stable wire/persist id)
    name  = "Foo",                  // the &'static str shown in the auto-rendered Features panel
    flags = FEATURE_DEFAULT_ON,     // FEATURE_DEFAULT_ON [ | FEATURE_ALWAYS_ON ]
    state = {
        /// Per-field docs are kept on the generated field.
        count: AtomicU8 = AtomicU8::new(0),  // one `field: Ty = const-init` per entry; `{}` if none
    },
    hooks = {
        // The overridden `Feature` hooks, spliced verbatim into `impl Feature for Foo`.
        // List only what you override; every other hook keeps its no-op default.
        fn active(&self) -> bool { self.count.load(Ordering::Relaxed) != 0 }
        fn on_report(&self, _c: &Ctx, mods: &mut u8, keys: &mut KeySet) { /* … */ }
    },
}
```

- **`state`** becomes the struct's private fields and the singleton's initialiser,
  one `field: Ty = init` per entry; each `init` must be `const` (the singleton is a
  `static`). A feature owning no state writes `state = {}`.
- **`hooks`** is spliced verbatim as the body of `impl Feature for Foo`, so each
  entry is an ordinary hook `fn` written exactly as inside a normal impl block.
  There is **no separate `impl Feature` for the generated methods** — the macro
  emits `id`/`name`/`flags`, and repeating any of them in `hooks` is a
  duplicate-definition compile error. Use `FEATURE_DEFAULT_ON`, not `DEFAULT_ON`.
- A feature's own helpers and press-edge entry points (Caps Word's `engage()`, Key
  Lock's `arm()`) stay in a normal `impl Foo { … }` block outside the macro.

The macro **does not** register the feature: the `FEATURES` array's order is
dispatch priority (a load-bearing invariant the linker cannot express), so the
entry is added by hand — or stamped by the scaffolder at the
`// @scaffold:features-registry` anchor — at the right position. Auto-registration
via `linkme`/`inventory` is deliberately avoided (it is invisible, and link order ≠
source order would forfeit the priority ordering). `caps_word.rs` and `key_lock.rs`
are the worked retrofits.

## The `just new-feature` scaffolder

`just new-feature <Name> --kind <toggle|config|keycode>` (a `cargo xtask`, sources
in `xtask/src/`) is the front door: one PascalCase name + a kind stamps the feature
file(s) and threads the feature through every wiring site, collapsing the ~17-edit,
~12-file scatter to *filling the marked `// TODO(behavior)`*. It:

- **Allocates** from the firmware's own resource tables, so a bad pick is a build
  error and never a silent bug: the next contiguous `FeatureId` (and re-points the
  `(FeatureId::… as u32) < 32` guard at the new highest variant); for `--kind config`
  the lowest free kcp group nibble (`0xB`/`0xC`/`0xE`), a config region chained in
  before `CRC_OFF`, and the `SCHEMA_VERSION` bump; for `--kind keycode` a suggested
  free `0xNN00` keycode window (printed, not wired — see below).
- **Stamps** `firmware/src/features/<name>.rs` from the kind's template — a complete
  `feature!{}` block with `// TODO(behavior)` at every body — and, for `config`, the
  app `kcp/<name>.ts` codec, `kcp/<name>.test.ts` drift test, and
  `featureDescriptors/<name>.ts` descriptor.
- **Wires** the firmware (the `mod`, the `FeatureId` variant, the `FEATURES` entry at
  the `// @scaffold:features-registry` anchor — appended last; reposition it for
  priority —, the `Cargo.toml` feature + `default`, the `save_feature`/`load_feature`
  arm) and, for `config`, the kcp group const + capability bit + dispatch arm +
  `CMD_*_GET`/`CMD_*_SET` opcodes, the persistence region + `serialize_`/
  `deserialize_`, and the app mirror (`protocol.ts` `Group`/`Cmd`, `info.ts`
  `GROUP_DEFS`, the `firmware-fixture` `CAPABILITIES`/`SCHEMA_VERSION`/dispatch, the
  `kcp/index.ts` re-export, and the descriptor's registration in
  `featureDescriptors/index.ts`).
- **Prints a checklist** of the only decisions left — the priority position, the
  hooks to implement, the field semantics — plus the validate command.

What it deliberately does **not** do:

- **`--kind keycode` is half-wired.** The registry side is stamped, but the keycode
  window itself (a `KeyAction` variant + decode in `keycode.rs`, a disjointness
  `assert!`, and the press-edge call in `keymap.rs`) is a genuine layout decision
  with no `@scaffold:` anchor — the checklist spells out the exact edits.
- **It does not rewrite test assertions.** A `--kind config` feature grows the
  protocol surface — a new bit in `CAPABILITIES` and a `SCHEMA_VERSION` bump — and
  several **hand-written** tests assert those as literals. The scaffolder cannot
  infer your intent there, so its checklist flags ~3 by-hand edits (the only
  hand-edits to *tests* in the whole flow):
  - `app/src/kcp/info.test.ts`: the two `CAPABILITIES` `0x….` assertions → the new
    mask; add the new group to `expectedPresent` (before `'features'`); the
    `schemaVersion` literal → the bumped value; and re-point the "unknown capability
    bit" example off your now-claimed nibble onto the next free one.
  - `app/src/kcp/codec.test.ts`: the "unknown group" example uses what was a free
    nibble — re-point it to the still-free one.

  The checklist prints the exact old→new numbers. After these, the generated drift
  tests + the build-time asserts (below) prove the rest in lockstep.

## kcp group and opcode conventions

kcp framing (`firmware/src/kcp.rs`): one 32-byte HID report per message, no report
id.

```text
request : [0]=CMD       [1]=SEQ  [2..32]=payload (30 bytes)
reply   : [0]=CMD|0x80  [1]=SEQ  [2]=STATUS  [3..32]=payload (29 bytes)
```

- `CMD` high nibble = the command **group**; low nibble = the **operation**. The
  group nibble is also the bit index of the group in the `CAPABILITIES` mask
  (present group `g` sets bit `g`).
- In `on_kcp`, `req` is the request payload (from byte 2) and `out` is the
  **29-byte** reply payload (from byte 3). `SEQ` pairs reply to request and keys
  chunked transfers.
- `Status` (reply byte 2): `Ok = 0`, `BadCmd = 1` (unknown op of a known group),
  `BadArg = 2` (bad/out-of-range argument), `Busy = 3`, `Unsupported = 4`
  (unknown group — also how capability negotiation degrades).

### Group nibbles — the allocation table

| Nibble | Group | Owner |
|---|---|---|
| `0x0` | INFO | core |
| `0x1` | KEYMAP | core |
| `0x2` | TELEMETRY | core |
| `0x3` | HID_KRO | core |
| `0x4` | CONFIG | core |
| `0x5` | MACRO | timed engine (`run_on_kcp`) |
| `0x6` | RGB | core |
| `0x7` | BEHAVIOR | SOCD / overrides / timed (`run_on_kcp`) |
| `0x8` | WIRELESS | core |
| `0x9` | TEXT | `autocorrect` feature (cfg-gated) |
| `0xA` | UNICODE | `unicode` feature (cfg-gated) |
| **`0xB`** | **free** | |
| **`0xC`** | **free** | |
| `0xD` | FEATURES | registry (`features_dispatch`) |
| **`0xE`** | **free** | |
| `0xF` | SYSTEM | core |

Only `0xB`, `0xC`, `0xE` are free. A fresh nibble is scarce — share the
BEHAVIOR/TEXT sub-opcode space unless your feature is a genuinely new domain.

### Wiring a feature-owned group

A `--kind config` feature that owns a group adds, on the firmware side:

1. `pub const FOO: u8 = 0xB;` in `kcp::group`.
2. `| (1 << group::FOO)` in `CAPABILITIES`.
3. a dispatch arm `group::FOO => features::run_on_kcp(cmd, req_payload, out),` in
   `handle`.
4. `pub const CMD_FOO_*: u8 = 0xB0;` opcodes (doc-commented with the exact
   payload layout — the doc comment *is* the spec the app mirror and the LLM
   read).
5. the `on_kcp` body, which follows this shape (model: `autocorrect.rs`):

```rust
fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
    let status = match cmd {
        kcp::CMD_FOO_GET => { /* fill out[..], 29 bytes max */ Status::Ok }
        kcp::CMD_FOO_SET => match req[0] { /* validate */ _ => Status::BadArg },
        _ if cmd >> 4 == kcp::group::FOO => Status::BadCmd, // known group, unknown op
        _ => return None,                                   // not our group; pass on
    };
    Some(status)
}
```

On the app side it adds: `Group.Foo`/`Cmd.Foo*` in `protocol.ts`, a `'foo'` row
in `info.ts` `GROUP_DEFS`, a `kcp/foo.ts` codec, the `firmware-fixture` row +
dispatch, and a `kcp/foo.test.ts` drift test. The scaffolder stamps these; the
drift check proves they match.

## FeatureDescriptor — the declarative GUI

A config feature's panel is **data**, not React. The scaffolder stamps one
descriptor file, `app/src/featureDescriptors/<name>.ts` (named for the feature; the
file `featureDescriptors/index.ts` registers), and the single generic
`app/src/ui/DescriptorPanel.tsx` renders it — seeding each control from its `get` op
on mount and writing via its `set` op on change (live-write + optimistic-revert, the
same idiom as TuningPanel / RgbPanel). The schema is
`app/src/featureDescriptors/types.ts`:

```ts
interface Op {
  cmd: number; // the raw kcp command byte (a `kcp/protocol.ts` `Cmd` value)
  args?: number[]; // fixed request-byte prefix: a `set` appends the value (`[...args, value]`);
  //                  a `get` sends them as-is (e.g. an index selecting a slot/field)
  at?: number; // GET-reply byte offset this control's value is decoded from (default 0);
  //              ignored by `set`
}
type Control =
  // each kind carries `label: string`, an `Op` `get`, an `Op` `set`, and the optional fields below
  | { kind: 'toggle' } //                          value 0|1     → Off/On segmented control
  | { kind: 'slider'; min; max; step? } //         bounded int   → range slider
  | { kind: 'number'; min; max } //                bounded int   → number input
  | { kind: 'enum'; options: { label; value }[] } // closed set  → <select>
  | { kind: 'color' }; //                          value [h,s,v] → HSV picker
// Every Control also has:
//   token?:  string  — a symbolic id, NOT sent on the wire; what another control's `showIf` names
//   showIf?: string  — visibility over token'd controls (==, !=, <, >, &&, ||, parens; a bare
//                      token is truthy when non-zero). A malformed expression OR an unknown token
//                      fails open (the control shows), per the LSP ignore-unknown rule.
interface FeatureDescriptor {
  fid: number;
  title: string;
  controls: Control[];
}
```

Why `at?` — the one field that makes this richer than a bare `{ cmd, args? }`: a
clean getter returns its value in reply byte 0, so `at` is omitted; but a feature
whose `GET` returns a **struct** names each field's offset (RGB `GET_STATE` puts
brightness at byte 4, so its control reads `at: 4`). The write mirror is `args`: a
`SET` that selects a field sends `[field, value]`, so the control writes
`set: { cmd: …Set, args: [field] }` and the runtime appends the value. The
scaffolded descriptor uses exactly this — one `GET`/`SET` op pair for the whole
group, each control reading byte `at` and writing field `args` — so adding a field
is one more control, no new opcode.

Render priority (the "ignore-unknown, don't crash" rule `info.ts`'s `unknownBits`
already embodies): a hand-built custom panel, else a registered descriptor, else the
**generic toggle** — so even a feature the app has never heard of still gets its
on/off switch. The registry (`featureDescriptors/index.ts`) ships **empty** — every
feature today has a richer hand-built panel — and a scaffolded `--kind config`
feature is the first to register a descriptor.

Choose control kinds so invalid states are unrepresentable: `enum` for a closed
choice set, `slider`/`number` with `min`/`max` for a bounded value, never a
free-form field. That is what makes a descriptor low-risk for a model to author.

## Config persistence

`firmware/src/config.rs` holds one fixed-offset, CRC-protected blob. Validity is
**exact-match** — magic + `SCHEMA_VERSION` + CRC32 must all match — so a version
bump cleanly invalidates an old blob to defaults; there is no migration.

### A toggle feature — nothing to do

The central enable bitmap is persisted for you, at `ENABLE_OFF` (4 bytes), by
`build_blob` (`put_u32(buf, ENABLE_OFF, features::enabled_map())`) and restored by
`restore_blob` (`features::set_enabled_map(get_u32(buf, ENABLE_OFF))`). A toggle
feature's only persistent state is its enable bit, which rides that word — **no
`config.rs` edit, no `SCHEMA_VERSION` bump.**

### A feature with its own params

Add, in `config.rs` (the scaffolder stubs these for `--kind config`):

1. a region before `CRC_OFF` — `const FOO_OFF = INDICATORS_OFF + …;` /
   `const FOO_BYTES = …;`, chaining off the previous region and shifting
   `CRC_OFF` (and so `BLOB_LEN`).
2. a `SCHEMA_VERSION` bump (note the reset-to-defaults consequence in the PR).
3. `serialize_foo`/`deserialize_foo` for the region's bytes.
4. `FeatureId::Foo` arms in `save_feature`/`load_feature` calling them.

Then implement `on_save`/`on_load` in `features/foo.rs`, delegating to
`config::save_feature(self.id(), out)` / `config::load_feature(self.id(), buf)`.
`build_blob` runs `features::run_on_save(buf)` and `restore_blob` runs
`features::run_on_load(buf)`, so the hooks are called for you. Persisting params
is the only path that bumps `SCHEMA_VERSION`.

## Keycode windows

`firmware/src/keycode.rs` partitions the 16-bit keycode space into disjoint
windows, decoded by one `Keycode::classify` → `KeyAction`. Relevant existing
windows and the gaps a new one fits into:

| Range | Meaning |
|---|---|
| `0x5A00..=0x5A08` | firmware behaviour controls (caps-word / key-lock / repeat / …) |
| `0x5A40..=0x5A42` | autocorrect controls |
| `0x5A50` / `0x5A51..=0x5A60` | unicode mode-cycle / `UM(n)` map slots |
| `0x5700..=0x57FF` | `TD(n)` tap-dance |
| `0x7700..=0x77FF` | `MACRO(n)` |

A `--kind keycode` feature carves a sub-window in a gap (e.g. extend the `0x5Axx`
behaviour-control block), adds a `KeyAction` variant + the decode arm in
`classify`, a disjointness `assert!`, and a press-edge call in
`keymap::compute_report` gated by `features::is_enabled(FeatureId::Foo)`.

## Build-time asserts — the collision guards

A resource collision is a **build error with a fix-it message**, never a silent
runtime bug. When one fires, read it: it names the constant to change.

| Assert (where) | Guards | When it fires / the fix |
|---|---|---|
| `assert!((FeatureId::Unicode as u32) < 32, "enable bitmap is a u32; widen ENABLED and config::ENABLE_BYTES past 32 features")` (`features/mod.rs`) | the `ENABLED` bitmap is a `u32`, so every `FeatureId` must be bit `< 32` | Update the guard to name your new highest `FeatureId`. Only if you genuinely reach 32 features do you widen `ENABLED` + `config::ENABLE_BYTES` (another schema bump). |
| `assert!(MSG_LEN - REPLY_PAYLOAD_IDX >= FOO_LEN, "… must fit one reply")` (`kcp.rs`, per group) | a reply payload fits the 29-byte frame | Your reply outgrew one frame: shrink it, or page it like `CMD_GET_FEATURES`/`CMD_RGB_LIST_MODES`. |
| `assert!(BLOB_LEN as u32 <= flash::CONFIG_REGION…)` (+ the `PAGES` variant) (`config.rs`) | the config blob fits the reserved flash region | Your config region pushed the blob past the reserved pages: shrink the region or the table caps. |
| the keycode-window disjointness `assert!` (`keycode.rs`) | keycode windows do not overlap | Your new window overlaps another: move it into a free gap. |

These are the discipline QMK's `_Static_assert(<= QK_COMMUNITY_MODULE_MAX)` uses,
generalised across all four resource axes (FeatureId · kcp nibble · keycode
window · config offset). Together with `npm run check:protocol` (the firmware↔app
drift check), they are the objective "done" signal the validation loop relies on
— see the [SKILL](../../.claude/skills/keeberry-feature/SKILL.md) step 4 and the
green gate in the [EXAMPLE](./EXAMPLE.md).
