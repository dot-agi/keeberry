<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# keeberry feature SDK — worked example

One `--kind config` feature, **DemoGizmo**, built end to end the way the
scaffolder really builds it: the `just new-feature` command, the files it stamps
(`feature!{}` block, descriptor, kcp codec, drift test — `// TODO(behavior)` at
every body), the two things you fill (the behaviour hook and the field schema),
the ~3 by-hand test edits its checklist names, and the green validation gate.
Copy this pattern; it exercises every axis — a behaviour hook, a kcp group, two
persisted params, a descriptor, and a drift test.

**DemoGizmo** is deliberately simple but real: while enabled, it asserts one
chosen HID modifier (config field 0, `modifier`, 0–7) on the report — on every
key, or only on letters (config field 1, `letters_only`). Its master on/off is the
automatic Features-panel toggle; its two params are a config panel rendered from a
descriptor.

## 1. Scaffold

```
just new-feature DemoGizmo --kind config
```

The scaffolder reads the firmware's own resource tables and allocates
`FeatureId::DemoGizmo = 9` (next free after `Unicode = 8`, re-pointing the `< 32`
enable-bitmap guard at it), the kcp group `0xB` (lowest free nibble →
`CMD_DEMO_GIZMO_GET = 0xB0` / `CMD_DEMO_GIZMO_SET = 0xB1`), and a 2-byte config
region chained in before `CRC_OFF` (bumping `SCHEMA_VERSION 10 → 11`). It stamps
`firmware/src/features/demo_gizmo.rs`, `app/src/kcp/demo_gizmo.ts` +
`demo_gizmo.test.ts`, and `app/src/featureDescriptors/demo_gizmo.ts`; wires the
registry, `Cargo.toml`, the kcp group/dispatch/opcodes, the config
serialize/deserialize, and the whole app mirror; and prints the checklist:

```text
Scaffolded `DemoGizmo` (--kind config) — allocated:
  • FeatureId::DemoGizmo = 9  (priority: appended last in FEATURES)
  • kcp group 0xB  (CMD_DEMO_GIZMO_GET 0xB0 / CMD_DEMO_GIZMO_SET 0xB1)
  • config region before CRC_OFF, SCHEMA_VERSION -> 11

Fill the TODO(behavior) markers, then decide:
  1. Priority — move the `&demo_gizmo::DEMO_GIZMO` line in features/mod.rs FEATURES …
  2. Hooks — implement the behaviour hook(s) in firmware/src/features/demo_gizmo.rs
     (on_kcp GET/SET + snapshot_config/restore_config are stubbed; add the effect hook).
  3. Fields — name the config bytes + ranges in features/demo_gizmo.rs, the codec in
     app/src/kcp/demo_gizmo.ts, and the controls in app/src/featureDescriptors/demo_gizmo.ts.
  4. Protocol-surface snapshots — a NEW kcp group + the SCHEMA bump change
     hard-coded literals the hand-written tests assert. Update:
       app/src/kcp/info.test.ts:
         - the two CAPABILITIES `0xa7ff` assertions -> 0xafff
         - add 'demoGizmo' to `expectedPresent` (before 'features')
         - the schemaVersion literal 10 -> 11
         - re-point the 'unknown capability bit' example off the now-claimed
           bit 0xB to the still-free 0xC.
       app/src/kcp/codec.test.ts:
         - the 'unknown group' example uses nibble 0xB (now claimed) —
           re-point it to the still-free 0xC.
```

Item 4 is the only place the scaffolder asks you to touch tests — it can't guess
the new mask/version literals, so it prints the exact old→new values (§6 below).

## 2. The generated firmware file (then filled)

`firmware/src/features/demo_gizmo.rs`, as stamped — the boilerplate is one
`feature!{}` block; the meaning is the `// TODO(behavior)` markers:

```rust
// SPDX-License-Identifier: GPL-2.0-or-later
//! Demo Gizmo — TODO(behavior): one sentence on what this feature does.
// … module docs + the Mutex/RefCell + kcp imports …

/// Persisted config-byte count — one per field. TODO(behavior): size to the real fields.
pub const CONFIG_LEN: usize = 2;

/// Factory-default config. TODO(behavior): set a sensible default per field.
const DEFAULT_CONFIG: [u8; CONFIG_LEN] = [0; CONFIG_LEN];

feature! {
    /// Demo Gizmo — TODO(behavior): summarise the feature and its fields.
    DemoGizmo as DEMO_GIZMO,
    id = FeatureId::DemoGizmo,
    name = "Demo Gizmo",
    flags = FEATURE_DEFAULT_ON,
    state = {
        /// The persisted config bytes: read/written over the kcp group, saved by config.
        config: Mutex<CriticalSectionRawMutex, RefCell<[u8; CONFIG_LEN]>> =
            Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new(DEFAULT_CONFIG)),
    },
    hooks = {
        /// Serve the DEMO_GIZMO kcp group (`0xBx`): `GET` returns the config bytes, `SET`
        /// writes one `[field, value]` pair. Any other group passes through (`None`).
        fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
            let status = match cmd {
                kcp::CMD_DEMO_GIZMO_GET => {
                    self.config.lock(|c| out[..CONFIG_LEN].copy_from_slice(&c.borrow()[..]));
                    Status::Ok
                }
                kcp::CMD_DEMO_GIZMO_SET => {
                    let (field, value) = (req[0] as usize, req[1]);
                    if field >= CONFIG_LEN {
                        Status::BadArg
                    } else {
                        // TODO(behavior): range-check `value` for this field before storing it.
                        self.config.lock(|c| c.borrow_mut()[field] = value);
                        Status::Ok
                    }
                }
                _ if cmd >> 4 == kcp::group::DEMO_GIZMO => Status::BadCmd,
                _ => return None,
            };
            Some(status)
        }

        fn on_save(&self, out: &mut [u8]) { crate::config::save_feature(self.id(), out); }
        fn on_load(&self, buf: &[u8]) { crate::config::load_feature(self.id(), buf); }

        // TODO(behavior): add the hook that *uses* the config — `on_report` to rewrite the
        // report, `on_matrix` to suppress keys, or `on_tick` for timed work — plus an
        // `active()` gate so an idle feature costs one relaxed load (see `caps_word.rs`).
    },
}

impl DemoGizmo {
    /// Snapshot half, called by config's `serialize_demo_gizmo`.
    pub fn snapshot_config(&self, region: &mut [u8]) {
        self.config.lock(|c| region[..CONFIG_LEN].copy_from_slice(&c.borrow()[..]));
    }

    /// Restore half, called by config's `deserialize_demo_gizmo`.
    pub fn restore_config(&self, region: &[u8]) {
        // TODO(behavior): validate each field before adopting a restored blob.
        self.config.lock(|c| c.borrow_mut()[..CONFIG_LEN].copy_from_slice(&region[..CONFIG_LEN]));
    }
}
```

Filled in — three changes, all at `TODO(behavior)`: add the effect hook (and the
imports + letter predicate it needs), range-check each field in the `SET` arm, and
validate on restore. Note there is **no** hand-written `impl Feature` and no
`id`/`name`/`flags` — `feature!` emits those; you only add the behaviour hook
inside `hooks = { … }`:

```rust
// add to the file's imports:
use crate::behavior::KeySet;
use crate::features::{Ctx, FeatureId, FEATURE_DEFAULT_ON};
use crate::keycode::KeyAction;

/// HID usages `a`–`z` (page 0x07), the letters `letters_only` restricts to.
const fn is_letter(usage: u8) -> bool { matches!(usage, 0x04..=0x1D) }

// inside hooks = { … }, replacing the trailing TODO comment:

        /// OR the chosen modifier into the report — on every key, or only when a letter is
        /// present. DemoGizmo has no idle state, so it leaves `active()` at its default; the
        /// dispatch gate already skips it while disabled.
        fn on_report(&self, _c: &Ctx, mods: &mut u8, keys: &mut KeySet) {
            let (modifier, letters_only) =
                self.config.lock(|c| { let cfg = c.borrow(); (cfg[0], cfg[1] != 0) });
            let bit = 1u8 << modifier;
            if !letters_only {
                *mods |= bit;
                return;
            }
            for kc in keys.as_slice() {
                if let KeyAction::Key(usage) = kc.classify() {
                    if is_letter(usage) {
                        *mods |= bit;
                        break;
                    }
                }
            }
        }

// in the SET arm, the range-check (field 0 = modifier 0–7, field 1 = flag 0|1):
                kcp::CMD_DEMO_GIZMO_SET => match (req[0], req[1]) {
                    (0, m) if m <= 7 => { self.config.lock(|c| c.borrow_mut()[0] = m); Status::Ok }
                    (1, f) if f <= 1 => { self.config.lock(|c| c.borrow_mut()[1] = f); Status::Ok }
                    _ => Status::BadArg, // unknown field or out-of-range value
                },

// in restore_config, clamp a stored modifier into range before adopting it:
        let modifier = region[0].min(7);
        let letters_only = u8::from(region[1] != 0);
        self.config.lock(|c| *c.borrow_mut() = [modifier, letters_only]);
```

## 3. The generated firmware wiring (one-liners, scaffolder-written)

```rust
// firmware/src/features/mod.rs
#[cfg(feature = "demo_gizmo")] pub mod demo_gizmo;       // module
    DemoGizmo = 9,                                        // FeatureId variant (next free)
    (FeatureId::DemoGizmo as u32) < 32,                   // the < 32 guard, now naming DemoGizmo
    #[cfg(feature = "demo_gizmo")] &demo_gizmo::DEMO_GIZMO, // FEATURES entry (last; reposition for priority)

// firmware/Cargo.toml
demo_gizmo = []                                           // + "demo_gizmo" added to `default`

// firmware/src/kcp.rs (cfg-gated on the feature)
pub const DEMO_GIZMO: u8 = 0xB;                           // group nibble (lowest free)
    | DEMO_GIZMO_CAP                                      // CAPABILITIES bit
    group::DEMO_GIZMO => features::run_on_kcp(cmd, req_payload, out), // dispatch arm
pub const CMD_DEMO_GIZMO_GET: u8 = 0xB0;                  // opcodes (doc-commented = the spec)
pub const CMD_DEMO_GIZMO_SET: u8 = 0xB1;
```

Persistence in `firmware/src/config.rs` — a 2-byte region chained in before
`CRC_OFF` (it takes `CRC_OFF`'s old offset; `CRC_OFF` re-chains past it), the
`SCHEMA_VERSION` bump, and the serialize/deserialize that delegate to the feature's
`snapshot_config`/`restore_config`:

```rust
pub const SCHEMA_VERSION: u16 = 11;                       // was 10 — invalidates saved configs
const DEMO_GIZMO_OFF: usize = ENABLE_OFF + ENABLE_BYTES; // = the old CRC_OFF expression, chained in
const DEMO_GIZMO_BYTES: usize = 2;
const CRC_OFF: usize = DEMO_GIZMO_OFF + DEMO_GIZMO_BYTES; // every later offset shifts down
// …
fn serialize_demo_gizmo(buf: &mut [u8]) {
    let region = &mut buf[DEMO_GIZMO_OFF..DEMO_GIZMO_OFF + DEMO_GIZMO_BYTES];
    features::demo_gizmo::DEMO_GIZMO.snapshot_config(region);
}
fn deserialize_demo_gizmo(buf: &[u8]) {
    let region = &buf[DEMO_GIZMO_OFF..DEMO_GIZMO_OFF + DEMO_GIZMO_BYTES];
    features::demo_gizmo::DEMO_GIZMO.restore_config(region);
}
// save_feature / load_feature gain `FeatureId::DemoGizmo => serialize_demo_gizmo(buf),` etc.
```

You wrote no `config.rs` offsets and no save/load arms by hand — only the
`snapshot_config`/`restore_config` bodies, in the feature file (§2).

## 4. The generated app codec

`app/src/kcp/demo_gizmo.ts` — thin `[field, value]` wire helpers mirroring the
firmware ops byte-for-byte (the drift check pairs each against the firmware):

```ts
// SPDX-License-Identifier: GPL-2.0-or-later
/** DEMO_GIZMO group (0xbx) wire helpers — Demo Gizmo. Mirrors `features::demo_gizmo`. */

/** Persisted config-byte count (mirror of `features::demo_gizmo::CONFIG_LEN`). */
export const DEMO_GIZMO_CONFIG_LEN = 2;

/** Parse a DEMO_GIZMO_GET reply: the first CONFIG_LEN bytes are the config fields. */
export function parseDemoGizmoConfig(payload: Uint8Array): { fields: number[] } {
  return { fields: Array.from(payload.slice(0, DEMO_GIZMO_CONFIG_LEN)) };
}

/** Build a DEMO_GIZMO_SET request payload `[field, value]`. */
export function encodeSetDemoGizmoField(field: number, value: number): number[] {
  return [field, value];
}
```

`protocol.ts` gains `Group.DemoGizmo = 0xb` and `Cmd.DemoGizmoGet = 0xb0` /
`DemoGizmoSet = 0xb1`; `info.ts` a `'demoGizmo'` `GROUP_DEFS` row; `kcp/index.ts`
the `export * from './demo_gizmo'`; and `firmware-fixture.ts` a `FEATURE_DEFS` row +
a `demoGizmoDispatch` holding the fake device's two config bytes — all
scaffolder-written.

## 5. The descriptor (the only GUI you write)

`app/src/featureDescriptors/demo_gizmo.ts` — the config panel as data, registered in
`featureDescriptors/index.ts` for you. The scaffolder stamps two placeholder
controls bound to the one `GET`/`SET` op pair, each reading reply byte `at` and
writing its field index through `args`; you rename them and pick the kinds/ranges:

```ts
// SPDX-License-Identifier: GPL-2.0-or-later
import { Cmd } from '../kcp/protocol';
import type { FeatureDescriptor } from './types';

/** `FeatureId::DemoGizmo` discriminant (`firmware/src/features/mod.rs`). */
export const FID_DEMO_GIZMO = 9;

export const demoGizmoDescriptor: FeatureDescriptor = {
  fid: FID_DEMO_GIZMO,
  title: 'Demo Gizmo',
  controls: [
    {
      kind: 'number', // was the stamped `slider`; a modifier index wants a small number input
      label: 'Modifier (0-7)',
      min: 0,
      max: 7,
      get: { cmd: Cmd.DemoGizmoGet, at: 0 }, // read config byte 0 from the GET reply
      set: { cmd: Cmd.DemoGizmoSet, args: [0] }, // SET field 0; the runtime appends the value
    },
    {
      kind: 'toggle',
      label: 'Letters only',
      get: { cmd: Cmd.DemoGizmoGet, at: 1 },
      set: { cmd: Cmd.DemoGizmoSet, args: [1] },
    },
  ],
};
```

The master DemoGizmo on/off needs no descriptor entry — it is the automatic toggle
in `FeaturesPanel`. No per-feature React: the one generic `DescriptorPanel` renders
this object.

## 6. The drift test + the hand-written snapshot edits

The scaffolder stamps `app/src/kcp/demo_gizmo.test.ts`, round-tripping the codec
through the in-repo firmware simulator (no hardware):

```ts
it('reads back a field written by DEMO_GIZMO_SET', () => {
  const device = createFakeDevice();
  const setReq = encodeRequest(Cmd.DemoGizmoSet, 1, encodeSetDemoGizmoField(0, 42));
  expect(decodeReply(fakeFirmwareHandle(setReq, device)).status).toBe(Status.Ok);
  const getReq = encodeRequest(Cmd.DemoGizmoGet, 2);
  const cfg = parseDemoGizmoConfig(decodeReply(fakeFirmwareHandle(getReq, device)).payload);
  expect(cfg.fields[0]).toBe(42);
});
```

DemoGizmo also appears, for free, in the existing `features.test.ts` enumeration.
Now make the ~3 by-hand edits the checklist (§1, item 4) named — the snapshot
literals a new group + schema bump moved:

```ts
// app/src/kcp/info.test.ts
expect(CAPABILITIES).toBe(0xafff); //   was 0xa7ff (group 0xB now lights bit 11)
expect(caps.raw).toBe(0xafff); //       was 0xa7ff
const expectedPresent: GroupName[] = [/* … */ 'unicode', 'demoGizmo', 'features', 'system'];
expect(info.schemaVersion).toBe(11); // was 10
expect(caps.unknownBits).toEqual([12]); // the unknown-bit example: 0xB is claimed, 0xC is free

// app/src/kcp/codec.test.ts
// the 'unknown group' example: swap its nibble 0xB → 0xC (still free)
```

## 7. Validate — the green gate

Run the loop the scaffolder printed (the build-time asserts + the drift check are
the objective done-signal, not inspection):

```text
$ (cd firmware && cargo build -p keeberry --release --target thumbv7m-none-eabi --features demo_gizmo)
    Finished `release` profile [optimized] target(s)
$ (cd firmware && cargo clippy -p keeberry --release --target thumbv7m-none-eabi --features demo_gizmo -- -D warnings)
    Finished — no warnings
$ (cd app && npm run format && npm test && npm run check:protocol && npm run build && npm run lint)
    format: all matched files use Prettier code style
     Test Files  N passed (N)
          Tests  M passed (M)
    kcp protocol in sync — N constants/groups/opcodes checked, all match.
    vite build — built in N.NNs
    eslint — no problems
```

All green means the build-time asserts (FeatureId `< 32`, the DEMO_GIZMO reply-fit,
the config-blob-fits-flash) passed and the firmware↔app wire format is in lockstep —
the objective "done", not inspection. In the PR, note that the `SCHEMA_VERSION
10 → 11` bump resets saved configs to defaults on next boot (DemoGizmo's two params
persist, which is the only reason the schema moved; a toggle-only feature would not
have touched it).
