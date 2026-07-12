// SPDX-License-Identifier: GPL-2.0-or-later
//! The files the scaffolder stamps, as `{{placeholder}}` templates.
//!
//! Each template is modelled on a real keeberry feature so the generated code matches the
//! house style byte-for-byte: the `toggle`/`keycode` feature files on `caps_word.rs`, the
//! `config` feature file on the group-owning, persisting plugins (`unicode.rs` + the config
//! persistence in `autocorrect.rs`), the TS codec/test on `kcp/autocorrect.{ts,test.ts}`, and
//! the descriptor on the `FeatureDescriptor` schema (`featureDescriptors/types.ts`). Every body
//! that carries real meaning is marked `// TODO(behavior):` so the contributor fills only the
//! value, never the wiring.

use crate::names::Names;
use crate::resources::Resources;

/// Substitute every `{{placeholder}}` for this feature into `template`.
pub fn render(template: &str, n: &Names, r: &Resources) -> String {
    let nibble = r.nibble.map(|x| format!("{x:x}")).unwrap_or_default();
    let mut s = template.to_string();
    for (k, v) in [
        ("{{Name}}", n.pascal.clone()),
        ("{{name}}", n.snake.clone()),
        ("{{NAME}}", n.screaming.clone()),
        ("{{Display}}", n.display.clone()),
        ("{{camel}}", n.camel.clone()),
        ("{{fid}}", r.feature_id.to_string()),
        ("{{nibble}}", nibble.clone()),
        ("{{cmd_get}}", format!("0x{nibble}0")),
        ("{{cmd_set}}", format!("0x{nibble}1")),
        ("{{config_len}}", CONFIG_LEN.to_string()),
    ] {
        s = s.replace(k, &v);
    }
    s
}

/// The persisted config-byte count the `config` kind generates (a slider field + a toggle
/// field). Mirrored on both sides (the firmware `CONFIG_LEN` and the TS codec) by `render`.
pub const CONFIG_LEN: usize = 2;

/// `--kind config` firmware feature: owns a kcp group (GET/SET its config), persists
/// [`CONFIG_LEN`] bytes. Modelled on `unicode.rs` (group-owning) + `autocorrect.rs` (persist).
pub const FEATURE_CONFIG: &str = r#"// SPDX-License-Identifier: GPL-2.0-or-later
//! {{Display}} — TODO(behavior): one sentence on what this feature does.
//!
//! Scaffolded by `just new-feature {{Name}} --kind config`. It owns the kcp group `0x{{nibble}}`
//! ([`CMD_{{NAME}}_GET`](crate::kcp::CMD_{{NAME}}_GET) / [`CMD_{{NAME}}_SET`](crate::kcp::CMD_{{NAME}}_SET))
//! and persists [`CONFIG_LEN`] config bytes through [`crate::config`]. Fill every `TODO(behavior)`
//! — the field meanings, their validation, and the hook that *uses* the config — then run the
//! validation loop the scaffolder printed.
//!
//! # Soundness
//!
//! The config lives behind a blocking [`Mutex`]/[`RefCell`], touched only synchronously, so no
//! borrow is ever held across an `.await` (the cooperative-executor rule every feature keeps).

use core::cell::RefCell;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;

use crate::features::{FeatureId, FEATURE_DEFAULT_ON};
use crate::kcp::{self, Status};

/// Persisted config-byte count — one per field. TODO(behavior): size to the real fields.
pub const CONFIG_LEN: usize = {{config_len}};

/// Factory-default config. TODO(behavior): set a sensible default per field.
const DEFAULT_CONFIG: [u8; CONFIG_LEN] = [0; CONFIG_LEN];

feature! {
    /// {{Display}} — TODO(behavior): summarise the feature and its fields.
    {{Name}} as {{NAME}},
    id = FeatureId::{{Name}},
    name = "{{Display}}",
    flags = FEATURE_DEFAULT_ON,
    state = {
        /// The persisted config bytes: read/written over the kcp group, saved by [`crate::config`].
        config: Mutex<CriticalSectionRawMutex, RefCell<[u8; CONFIG_LEN]>> =
            Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new(DEFAULT_CONFIG)),
    },
    hooks = {
        /// Serve the {{NAME}} kcp group (`0x{{nibble}}x`): `GET` returns the config bytes, `SET`
        /// writes one `[field, value]` pair. Any other group passes through (`None`).
        fn on_kcp(&self, cmd: u8, req: &[u8], out: &mut [u8]) -> Option<Status> {
            let status = match cmd {
                kcp::CMD_{{NAME}}_GET => {
                    self.config.lock(|c| out[..CONFIG_LEN].copy_from_slice(&c.borrow()[..]));
                    Status::Ok
                }
                kcp::CMD_{{NAME}}_SET => {
                    let (field, value) = (req[0] as usize, req[1]);
                    if field >= CONFIG_LEN {
                        Status::BadArg
                    } else {
                        // TODO(behavior): range-check `value` for this field before storing it.
                        self.config.lock(|c| c.borrow_mut()[field] = value);
                        Status::Ok
                    }
                }
                // Known group, unrecognised operation.
                _ if cmd >> 4 == kcp::group::{{NAME}} => Status::BadCmd,
                _ => return None,
            };
            Some(status)
        }

        /// Persist this feature's config region (delegates to [`crate::config::save_feature`]).
        fn on_save(&self, out: &mut [u8]) {
            crate::config::save_feature(self.id(), out);
        }

        /// Restore this feature's config region (delegates to [`crate::config::load_feature`]).
        fn on_load(&self, buf: &[u8]) {
            crate::config::load_feature(self.id(), buf);
        }

        // TODO(behavior): add the hook that *uses* the config — `on_report` to rewrite the
        // report, `on_matrix` to suppress keys, or `on_tick` for timed work — plus an
        // `active()` gate so an idle feature costs one relaxed load (see `caps_word.rs`).
    },
}

impl {{Name}} {
    /// Copy the live config into `region` ([`CONFIG_LEN`] bytes) — the snapshot half called by
    /// [`crate::config`]'s `serialize_{{name}}`.
    pub fn snapshot_config(&self, region: &mut [u8]) {
        self.config.lock(|c| region[..CONFIG_LEN].copy_from_slice(&c.borrow()[..]));
    }

    /// Adopt a restored `region` ([`CONFIG_LEN`] bytes) — the restore half called by
    /// [`crate::config`]'s `deserialize_{{name}}`.
    pub fn restore_config(&self, region: &[u8]) {
        // TODO(behavior): validate each field before adopting a restored blob.
        self.config.lock(|c| c.borrow_mut()[..CONFIG_LEN].copy_from_slice(&region[..CONFIG_LEN]));
    }
}
"#;

/// `--kind toggle` firmware feature: a runtime-toggleable behaviour, no kcp/config surface.
/// Appears in the Features panel for free. Modelled on `caps_word.rs`.
pub const FEATURE_TOGGLE: &str = r#"// SPDX-License-Identifier: GPL-2.0-or-later
//! {{Display}} — TODO(behavior): one sentence on what this feature does.
//!
//! Scaffolded by `just new-feature {{Name}} --kind toggle`: a runtime-toggleable behaviour with
//! no kcp config surface. It is enumerated and switched on/off over the `FEATURES` kcp group and
//! rendered as one switch in the Features panel — zero GUI code. Fill the `TODO(behavior)` hook
//! (or swap it for the one the behaviour needs) and gate it on [`Feature::active`] if it carries
//! idle state. `caps_word.rs` is the worked model.
//!
//! # Soundness
//!
//! Every hook is synchronous and holds no borrow across an `.await`, like every other feature.

use crate::behavior::KeySet;
use crate::features::{Ctx, FeatureId, FEATURE_DEFAULT_ON};

feature! {
    /// {{Display}} — TODO(behavior): summarise the feature.
    {{Name}} as {{NAME}},
    id = FeatureId::{{Name}},
    name = "{{Display}}",
    flags = FEATURE_DEFAULT_ON,
    state = {},
    hooks = {
        /// TODO(behavior): transform the resolved report while this feature is enabled, or
        /// replace this with the hook the behaviour needs (`on_matrix`/`on_overlay`/`on_tick`).
        fn on_report(&self, _c: &Ctx, _mods: &mut u8, _keys: &mut KeySet) {}
    },
}
"#;

/// `--kind keycode` firmware feature: a behaviour fired from a dedicated keycode's press edge.
/// The registry wiring is generated; the keycode window itself is carved by hand (keycode.rs /
/// keymap.rs carry no `@scaffold:` anchor — it is a genuine layout decision the checklist spells
/// out, with the reserved window the scaffolder picked). Modelled on `caps_word.rs`.
pub const FEATURE_KEYCODE: &str = r#"// SPDX-License-Identifier: GPL-2.0-or-later
//! {{Display}} — TODO(behavior): one sentence on what this feature does.
//!
//! Scaffolded by `just new-feature {{Name}} --kind keycode`: a behaviour triggered by a
//! dedicated keycode's press edge (the `caps_word.rs` shape). The registry wiring is done; the
//! keycode itself is the one part with no `@scaffold:` anchor, so the printed checklist lists the
//! exact keycode.rs / keymap.rs edits — carve the reserved window, add the `KeyAction` variant +
//! decode, and call this feature's press-edge method from `keymap::compute_report` (gated on
//! [`crate::features::is_enabled`]).
//!
//! # Soundness
//!
//! Every hook is synchronous and holds no borrow across an `.await`, like every other feature.

use crate::behavior::KeySet;
use crate::features::{Ctx, FeatureId, FEATURE_DEFAULT_ON};

feature! {
    /// {{Display}} — TODO(behavior): summarise the feature.
    {{Name}} as {{NAME}},
    id = FeatureId::{{Name}},
    name = "{{Display}}",
    flags = FEATURE_DEFAULT_ON,
    state = {},
    hooks = {
        /// TODO(behavior): act on the keycode trigger (see the checklist for the keycode wiring),
        /// or switch to the hook the behaviour needs (`on_matrix`/`on_overlay`/`on_tick`).
        fn on_report(&self, _c: &Ctx, _mods: &mut u8, _keys: &mut KeySet) {}
    },
}
"#;

/// `--kind config` TS wire codec (`app/src/kcp/<name>.ts`). Modelled on `kcp/autocorrect.ts`.
pub const APP_CODEC: &str = r#"// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * {{NAME}} group (0x{{nibble}}x) wire helpers — {{Display}}. Mirrors the firmware
 * `features::{{name}}` dispatch byte-for-byte. TODO(behavior): document the real fields.
 *
 * Ops (low nibble of CMD):
 *  - {{NAME}}_GET ({{cmd_get}}): no request payload; reply `[field0, field1, …]` (CONFIG_LEN bytes).
 *  - {{NAME}}_SET ({{cmd_set}}): request `[field, value]`; an out-of-range field answers BadArg.
 */

/** Persisted config-byte count (mirror of `features::{{name}}::CONFIG_LEN`). */
export const {{NAME}}_CONFIG_LEN = {{config_len}};

/** The {{Display}} config a {{NAME}}_GET reply carries. TODO(behavior): name the fields. */
export interface {{Name}}Config {
  /** The raw config bytes, one per field. */
  fields: number[];
}

/** Parse a {{NAME}}_GET reply: the first CONFIG_LEN bytes are the config fields. */
export function parse{{Name}}Config(payload: Uint8Array): {{Name}}Config {
  return { fields: Array.from(payload.slice(0, {{NAME}}_CONFIG_LEN)) };
}

/** Build a {{NAME}}_SET request payload `[field, value]`. */
export function encodeSet{{Name}}Field(field: number, value: number): number[] {
  return [field, value];
}
"#;

/// `--kind config` drift test (`app/src/kcp/<name>.test.ts`), round-tripping the codec through
/// the firmware fixture exactly as `kcp/autocorrect.test.ts` does.
pub const APP_TEST: &str = r#"// SPDX-License-Identifier: GPL-2.0-or-later
import { describe, expect, it } from 'vitest';
import { decodeReply, encodeRequest } from './codec';
import { Cmd, Status } from './protocol';
import { {{NAME}}_CONFIG_LEN, encodeSet{{Name}}Field, parse{{Name}}Config } from './{{name}}';
import { createFakeDevice, fakeFirmwareHandle } from './firmware-fixture';

describe('{{Name}} codec (mirror of the features::{{name}} dispatch)', () => {
  it('parses the config fields and encodes a [field, value] set', () => {
    expect(parse{{Name}}Config(new Uint8Array([7, 1, 0]))).toEqual({ fields: [7, 1] });
    expect(encodeSet{{Name}}Field(1, 1)).toEqual([1, 1]);
  });
});

describe('{{Name}} dispatch through the codec (set/get round-trips)', () => {
  it('reads back a field written by {{NAME}}_SET', () => {
    const device = createFakeDevice();
    const setReq = encodeRequest(Cmd.{{Name}}Set, 1, encodeSet{{Name}}Field(0, 42));
    expect(decodeReply(fakeFirmwareHandle(setReq, device)).status).toBe(Status.Ok);
    const getReq = encodeRequest(Cmd.{{Name}}Get, 2);
    const cfg = parse{{Name}}Config(decodeReply(fakeFirmwareHandle(getReq, device)).payload);
    expect(cfg.fields[0]).toBe(42);
  });

  it('rejects an out-of-range field with BadArg', () => {
    const device = createFakeDevice();
    const badReq = encodeRequest(Cmd.{{Name}}Set, 1, [{{NAME}}_CONFIG_LEN, 1]);
    expect(decodeReply(fakeFirmwareHandle(badReq, device)).status).toBe(Status.BadArg);
  });
});
"#;

/// `--kind config` self-describing GUI descriptor (`app/src/featureDescriptors/<name>.ts`) —
/// rendered by the one generic `DescriptorPanel`, no per-feature React. Modelled on the
/// `FeatureDescriptor` schema in `featureDescriptors/types.ts`.
pub const APP_DESCRIPTOR: &str = r#"// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * {{Display}} (`FeatureId::{{Name}}`) as a `FeatureDescriptor` — its config GUI as data, drawn
 * by the generic `DescriptorPanel`. TODO(behavior): replace these placeholder controls with the
 * feature's real fields. The {{NAME}} group's GET ({{cmd_get}}) returns the config bytes and SET
 * ({{cmd_set}}) takes `[field, value]`, so each control reads reply byte `at` and writes its field
 * index through `args`.
 */
import { Cmd } from '../kcp/protocol';
import type { FeatureDescriptor } from './types';

/** `FeatureId::{{Name}}` discriminant (`firmware/src/features/mod.rs`). */
export const FID_{{NAME}} = {{fid}};

export const {{camel}}Descriptor: FeatureDescriptor = {
  fid: FID_{{NAME}},
  title: '{{Display}}',
  controls: [
    // TODO(behavior): name the fields and pick the right control kinds / ranges.
    {
      kind: 'slider',
      label: 'Field 0',
      min: 0,
      max: 255,
      get: { cmd: Cmd.{{Name}}Get, at: 0 },
      set: { cmd: Cmd.{{Name}}Set, args: [0] },
    },
    {
      kind: 'toggle',
      label: 'Field 1',
      get: { cmd: Cmd.{{Name}}Get, at: 1 },
      set: { cmd: Cmd.{{Name}}Set, args: [1] },
    },
  ],
};
"#;
