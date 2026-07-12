// SPDX-License-Identifier: GPL-2.0-or-later
/**
 * kcp protocol drift-check.
 *
 * The firmware (`firmware/src/kcp.rs` + `config.rs` + `usb.rs`) is the
 * SINGLE SOURCE OF TRUTH for the kcp wire format; the TS client mirrors it by hand
 * (`app/src/kcp/protocol.ts`, `firmware-fixture.ts`, `info.ts`). Those two can
 * silently drift. This script extracts the shared constants from both sides and
 * exits non-zero on any divergence, so CI catches a firmware<->app mismatch
 * before it ships. (A full codegen from one source is a possible future step;
 * this lightweight regex check is the v1 contract — see `protocol/README.md`.)
 *
 * Coverage: the framing offsets, `REPLY_FLAG`/`MSG_LEN`, the `Status`
 * discriminants, the command-group bit indices, the `CAPABILITIES` mask, the
 * USB usage page/usage, `SCHEMA_VERSION`, `DEVICE_INFO_LEN`, the config
 * persistence window (base/size/pages, derived from the firmware flash layout),
 * and a NAME->VALUE 1:1 pairing of every command opcode (so a *swap* of two
 * opcodes — same value set, inverted meaning — is caught, which a set comparison
 * would miss).
 */
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '..', '..'); // app/scripts -> repo root
const read = (p) => readFileSync(resolve(repo, p), 'utf8');

const fw = read('firmware/src/kcp.rs');
const fwConfig = read('firmware/src/config.rs');
const fwFlash = read('firmware/src/flash.rs');
const fwUsb = read('firmware/src/usb.rs');
const fwKeycode = read('firmware/src/keycode.rs');
const fwBuild = read('firmware/build.rs');
const ts = read('app/src/kcp/protocol.ts');
const tsFixture = read('app/src/kcp/firmware-fixture.ts');
const tsInfo = read('app/src/kcp/info.ts');
const tsKeycode = read('app/src/kcp/keycode.ts');

const drift = [];
const checked = [];

const toNum = (s) => (s == null ? null : Number(s));
const hex = (v) => (v == null ? String(v) : '0x' + v.toString(16).padStart(2, '0'));
const normKey = (k) => k.toUpperCase().replace(/_/g, ''); // HID_KRO == HidKro

function extract(text, re, label) {
  const m = text.match(re);
  if (!m) {
    drift.push(`could not extract ${label} — the source moved; update this check`);
    return null;
  }
  return toNum(m[1]);
}
// Parse an integer constant value that may carry digit separators (`0x0801_f400`)
// or be a simple `A * B` product (flash.rs `FLASH_SIZE = 128 * 1024`); returns null
// for anything else so the caller reports it as drift rather than guessing.
function evalIntExpr(s) {
  if (s == null) return null;
  const c = s.replace(/_/g, '').trim();
  if (/^(0x[0-9a-f]+|\d+)$/i.test(c)) return Number(c);
  const mul = c.match(/^(\d+)\s*\*\s*(\d+)$/);
  return mul ? Number(mul[1]) * Number(mul[2]) : null;
}
function extractExpr(text, re, label) {
  const m = text.match(re);
  if (!m) {
    drift.push(`could not extract ${label} — the source moved; update this check`);
    return null;
  }
  return evalIntExpr(m[1]);
}
function compare(name, fwVal, tsVal) {
  checked.push(name);
  if (fwVal == null || tsVal == null || fwVal !== tsVal) {
    drift.push(`${name}: firmware=${fwVal} vs app=${tsVal}`);
  }
}

// --- scalar framing/version constants (firmware name -> TS name) ---
compare(
  'MSG_LEN',
  extract(fw, /pub const MSG_LEN:\s*usize\s*=\s*(\d+)/, 'firmware MSG_LEN'),
  extract(ts, /export const MSG_LEN\s*=\s*(\d+)/, 'app MSG_LEN'),
);
compare(
  'REPLY_FLAG',
  extract(fw, /pub const REPLY_FLAG:\s*u8\s*=\s*(0x[0-9a-fA-F]+)/, 'firmware REPLY_FLAG'),
  extract(ts, /export const REPLY_FLAG\s*=\s*(0x[0-9a-fA-F]+)/, 'app REPLY_FLAG'),
);
compare(
  'CMD_IDX',
  extract(fw, /const CMD_IDX:\s*usize\s*=\s*(\d+)/, 'firmware CMD_IDX'),
  extract(ts, /export const CMD_IDX\s*=\s*(\d+)/, 'app CMD_IDX'),
);
compare(
  'SEQ_IDX',
  extract(fw, /const SEQ_IDX:\s*usize\s*=\s*(\d+)/, 'firmware SEQ_IDX'),
  extract(ts, /export const SEQ_IDX\s*=\s*(\d+)/, 'app SEQ_IDX'),
);
compare(
  'STATUS_IDX',
  extract(fw, /const STATUS_IDX:\s*usize\s*=\s*(\d+)/, 'firmware STATUS_IDX'),
  extract(ts, /export const STATUS_IDX\s*=\s*(\d+)/, 'app STATUS_IDX'),
);
compare(
  'REQ_PAYLOAD_IDX',
  extract(fw, /const REQ_PAYLOAD_IDX:\s*usize\s*=\s*(\d+)/, 'firmware REQ_PAYLOAD_IDX'),
  extract(ts, /export const REQ_PAYLOAD_IDX\s*=\s*(\d+)/, 'app REQ_PAYLOAD_IDX'),
);
compare(
  'REPLY_PAYLOAD_IDX',
  extract(fw, /const REPLY_PAYLOAD_IDX:\s*usize\s*=\s*(\d+)/, 'firmware REPLY_PAYLOAD_IDX'),
  extract(ts, /export const REPLY_PAYLOAD_IDX\s*=\s*(\d+)/, 'app REPLY_PAYLOAD_IDX'),
);
compare(
  'DEVICE_INFO_LEN',
  extract(fw, /const DEVICE_INFO_LEN:\s*usize\s*=\s*(\d+)/, 'firmware DEVICE_INFO_LEN'),
  extract(tsInfo, /DEVICE_INFO_LEN\s*=\s*(\d+)/, 'app DEVICE_INFO_LEN'),
);
compare(
  'SCHEMA_VERSION',
  extract(fwConfig, /SCHEMA_VERSION:\s*u16\s*=\s*(\d+)/, 'firmware SCHEMA_VERSION'),
  extract(tsFixture, /export const SCHEMA_VERSION\s*=\s*(\d+)/, 'app SCHEMA_VERSION'),
);

// --- config persistence window: the firmware derives `CONFIG_REGION` from
//     FLASH_BASE/FLASH_SIZE/CONFIG_PAGES/PAGE_SIZE (flash.rs); the app fixture
//     (firmware-fixture.ts) hard-codes the resulting base/size that `GET_STORAGE_INFO`
//     reports. Re-derive the window from the firmware constants and compare base, size
//     and the reserved page count, so a change to `CONFIG_PAGES` (and thus the address
//     range the GUI shows) can never silently diverge from the client again. ---
{
  const flashBase = extractExpr(
    fwFlash,
    /pub const FLASH_BASE:\s*u32\s*=\s*([^;]+);/,
    'firmware FLASH_BASE',
  );
  const flashSize = extractExpr(
    fwFlash,
    /pub const FLASH_SIZE:\s*u32\s*=\s*([^;]+);/,
    'firmware FLASH_SIZE',
  );
  const pageSize = extractExpr(
    fwFlash,
    /pub const PAGE_SIZE:\s*usize\s*=\s*([^;]+);/,
    'firmware PAGE_SIZE',
  );
  const configPages = extractExpr(
    fwFlash,
    /pub const CONFIG_PAGES:\s*u32\s*=\s*([^;]+);/,
    'firmware CONFIG_PAGES',
  );
  const appBase = extractExpr(
    tsFixture,
    /export const CONFIG_REGION_BASE\s*=\s*([^;]+);/,
    'app CONFIG_REGION_BASE',
  );
  const appSize = extractExpr(
    tsFixture,
    /export const CONFIG_REGION_SIZE\s*=\s*([^;]+);/,
    'app CONFIG_REGION_SIZE',
  );
  const derived = [flashBase, flashSize, pageSize, configPages].every((v) => v != null);
  const fwSize = derived ? configPages * pageSize : null;
  const fwBase = derived ? flashBase + flashSize - fwSize : null;
  compare('CONFIG_REGION_BASE', fwBase, appBase);
  compare('CONFIG_REGION_SIZE', fwSize, appSize);
  // The reserved page count drives both the base and size above; check it directly
  // (app size / firmware page size) so a page-count change gives a clear signal.
  compare(
    'CONFIG_REGION_PAGES',
    configPages,
    appSize != null && pageSize ? appSize / pageSize : null,
  );
}

// --- USB raw-HID identity: the usage page/usage the kcp interface lives on, read
//     from the firmware's report descriptor bytes (usb.rs) vs protocol.ts. The
//     descriptor scopes the search so the other HID descriptors' usage items
//     (keyboard, consumer, …) cannot be mistaken for the kcp interface's. ---
{
  const desc = fwUsb.match(/const KCP_HID_DESCRIPTOR:\s*&\[u8\]\s*=\s*&\[([\s\S]*?)\];/);
  let fwUsagePage = null;
  let fwUsage = null;
  if (!desc) {
    drift.push(
      'could not extract firmware KCP_HID_DESCRIPTOR — the source moved; update this check',
    );
  } else {
    // Usage Page is the 2-byte item `0x06, lo, hi` (little-endian); Usage is the
    // first `0x09, nn` — the vendor interface usage (0x61), ahead of the IN/OUT
    // report usages (0x62/0x63).
    const up = desc[1].match(/0x06,\s*0x([0-9a-fA-F]{2}),\s*0x([0-9a-fA-F]{2})/);
    const ug = desc[1].match(/0x09,\s*0x([0-9a-fA-F]{2})/);
    if (up) fwUsagePage = parseInt(up[1], 16) | (parseInt(up[2], 16) << 8);
    if (ug) fwUsage = parseInt(ug[1], 16);
  }
  compare(
    'USAGE_PAGE',
    fwUsagePage,
    extract(ts, /export const USAGE_PAGE\s*=\s*(0x[0-9a-fA-F]+)/, 'app USAGE_PAGE'),
  );
  compare('USAGE', fwUsage, extract(ts, /export const USAGE\s*=\s*(0x[0-9a-fA-F]+)/, 'app USAGE'));
}

// --- `enum Status` discriminants (identical Rust + TS syntax: `Ident = N,`) ---
function enumBody(text, name) {
  const block = text.match(new RegExp(`enum ${name}\\s*\\{([^}]*)\\}`));
  const out = {};
  if (block) for (const m of block[1].matchAll(/(\w+)\s*=\s*(\d+)/g)) out[m[1]] = toNum(m[2]);
  return out;
}
{
  const a = enumBody(fw, 'Status');
  const b = enumBody(ts, 'Status');
  for (const k of new Set([...Object.keys(a), ...Object.keys(b)]))
    compare(`Status.${k}`, a[k], b[k]);
}

// --- command group bit indices (firmware `pub mod group`; TS `Group` object) ---
function fwGroupBits(text) {
  const block = text.match(/pub mod group\s*\{([\s\S]*?)\n\}/);
  const out = {};
  if (block)
    for (const m of block[1].matchAll(/pub const (\w+):\s*u8\s*=\s*(0x[0-9a-fA-F]+)/g))
      out[normKey(m[1])] = toNum(m[2]);
  return out;
}
function tsConstObject(text, name) {
  const block = text.match(
    new RegExp(`export const ${name}\\s*=\\s*\\{([\\s\\S]*?)\\}\\s*as const`),
  );
  const out = {};
  if (block)
    for (const m of block[1].matchAll(/(\w+):\s*(0x[0-9a-fA-F]+|\d+)/g))
      out[normKey(m[1])] = toNum(m[2]);
  return out;
}
// Computed once and reused by both the group-bit check and CAPABILITIES below.
const fwBits = fwGroupBits(fw);
const tsGroupBits = tsConstObject(ts, 'Group');
for (const k of new Set([...Object.keys(fwBits), ...Object.keys(tsGroupBits)]))
  compare(`group.${k}`, fwBits[k], tsGroupBits[k]);

// --- CAPABILITIES mask: evaluate both OR-expressions over the parsed group bits
//     and compare the resulting value (firmware `(1 << group::X) | …`; TS fixture
//     `(1 << Group.X) | …`). Mirrors a constant the README claims but was unchecked. ---
function maskFromExpr(expr, refRe, bits) {
  let mask = 0;
  const re = new RegExp(refRe, 'g');
  let m;
  while ((m = re.exec(expr))) {
    const bit = bits[normKey(m[1])];
    if (bit == null) return null; // references a group we did not parse
    mask |= 1 << bit;
  }
  return mask >>> 0;
}
// A feature-gated capability bit is factored into its own `const NAME: u32 = 1 << group::X;`
// so a `#[cfg(feature = "…")]` can gate it (an attribute cannot sit on a `|` operand), then it
// is OR-ed into CAPABILITIES by name (`TEXT_CAPABILITY`, `UNICODE_CAP`). Inline each such
// `_CAP`-rooted reference with its enabled definition (the first, `#[cfg(feature = "…")]` arm)
// so the mask reflects the default build; the `group::X` terms carry no `: u32` const and so
// pass through untouched for `maskFromExpr` to resolve.
function inlineCapConsts(expr, text) {
  return expr.replace(/\b\w+_CAP\w*\b/g, (token) => {
    const def = text.match(new RegExp(`const ${token}:\\s*u32\\s*=\\s*([^;]+);`));
    return def ? def[1] : token;
  });
}
{
  const fwExpr = fw.match(/const CAPABILITIES:\s*u32\s*=([\s\S]*?);/);
  const tsExpr = tsFixture.match(/export const CAPABILITIES\s*=([\s\S]*?);/);
  if (!fwExpr)
    drift.push('could not extract firmware CAPABILITIES — the source moved; update this check');
  if (!tsExpr)
    drift.push('could not extract app CAPABILITIES — the source moved; update this check');
  // TEXT and UNICODE are the optional groups: each is OR-ed into CAPABILITIES through a
  // cfg-gated `*_CAP` helper (`1 << group::X` when its owning feature is built in) rather than
  // the bit inline, so `inlineCapConsts` substitutes every `*_CAP` with its enabled definition
  // before the mask is read — folding both bits in to match the app fixture's default build.
  compare(
    'CAPABILITIES',
    fwExpr ? maskFromExpr(inlineCapConsts(fwExpr[1], fw), 'group::(\\w+)', fwBits) : null,
    tsExpr ? maskFromExpr(tsExpr[1], 'Group\\.(\\w+)', tsGroupBits) : null,
  );
}

// --- command opcodes: NAME->VALUE 1:1 pairing. Each TS `Cmd.<Name>` maps to a
//     firmware `CMD_<NAME>` (PascalCase -> UPPER_SNAKE) and their values must be
//     equal — so a *swap* of two opcodes (e.g. SocdSet<->SocdClear: same value
//     set, inverted meaning) FAILS, which the old value-set comparison would miss. ---
{
  // Pair opcodes by an underscore-insensitive, case-insensitive key (`normKey`, the
  // same normalization used for groups), so firmware `CMD_DEMO_A_GET` pairs with app
  // `Cmd.DemoAGet` regardless of where the snake/camel word boundaries land — a
  // single-letter word (`DemoA`) otherwise misaligns under a camel→snake transform.
  const fwCmds = {};
  for (const m of fw.matchAll(/pub const CMD_([A-Z0-9_]+):\s*u8\s*=\s*(0x[0-9a-fA-F]+)/g))
    fwCmds[normKey(m[1])] = { value: toNum(m[2]), fwName: m[1] };

  const tsBlock = ts.match(/export const Cmd\s*=\s*\{([\s\S]*?)\}\s*as const/);
  const tsCmds = {};
  if (!tsBlock) {
    drift.push('could not extract app Cmd opcodes — the source moved; update this check');
  } else {
    // Strip JSDoc and line comments first, so hex written inside `/** … 0xNN … */`
    // is never mistaken for an opcode entry — only real `Name: 0xNN` survive.
    const body = tsBlock[1].replace(/\/\*\*[\s\S]*?\*\//g, '').replace(/\/\/[^\n]*/g, '');
    for (const m of body.matchAll(/(\w+)\s*:\s*(0x[0-9a-fA-F]+)/g))
      tsCmds[normKey(m[1])] = { value: toNum(m[2]), tsName: m[1] };
  }

  for (const key of [...new Set([...Object.keys(fwCmds), ...Object.keys(tsCmds)])].sort()) {
    const fwCmd = fwCmds[key];
    const tsCmd = tsCmds[key];
    checked.push(tsCmd ? `Cmd.${tsCmd.tsName}` : `CMD_${fwCmd.fwName}`);
    if (!fwCmd) {
      drift.push(`Cmd.${tsCmd.tsName} (${hex(tsCmd.value)}) has no firmware CMD_* counterpart`);
    } else if (!tsCmd) {
      drift.push(`CMD_${fwCmd.fwName} (${hex(fwCmd.value)}) has no app Cmd.* counterpart`);
    } else if (fwCmd.value !== tsCmd.value) {
      drift.push(
        `opcode ${fwCmd.fwName}: firmware=${hex(fwCmd.value)} vs app Cmd.${tsCmd.tsName}=${hex(tsCmd.value)}`,
      );
    }
  }
}

// --- autocorrect control keycodes (`AUTOCORRECT_TOGGLE`/`AUTOCORRECT_ON`/`AUTOCORRECT_OFF`): the firmware derives the
//     three from `AUTOCORRECT_KC_BASE` (toggle/on/off = base+0/+1/+2, `keycode.rs`); the app
//     hard-codes them (`keycode.ts`). A shift of the base or a reordering diverges the bound
//     keys the GUI writes from what the firmware decodes. ---
{
  const base = extract(
    fwKeycode,
    /const AUTOCORRECT_KC_BASE:\s*u16\s*=\s*(0x[0-9a-fA-F]+)/,
    'firmware AUTOCORRECT_KC_BASE',
  );
  const acKeys = [
    ['AUTOCORRECT_TOGGLE', 0],
    ['AUTOCORRECT_ON', 1],
    ['AUTOCORRECT_OFF', 2],
  ];
  for (const [name, off] of acKeys) {
    compare(
      name,
      base == null ? null : base + off,
      extract(
        tsKeycode,
        new RegExp(`export const ${name}\\s*=\\s*(0x[0-9a-fA-F]+)`),
        `app ${name}`,
      ),
    );
  }
}

// --- autocorrect dictionary size: the compiled-in `DICT` (typo, correction) pair count in
//     `build.rs` vs `AUTOCORRECT_ENTRY_COUNT` in the app fixture, which mirrors what the
//     `AUTOCORRECT_INFO` reply reports. Each `DICT` entry opens with `("`. ---
{
  const dictBlock = fwBuild.match(/const DICT:\s*&\[\(&str,\s*&str\)\]\s*=\s*&\[([\s\S]*?)\];/);
  let fwDictCount = null;
  if (!dictBlock) {
    drift.push('could not extract firmware DICT — the source moved; update this check');
  } else {
    fwDictCount = (dictBlock[1].match(/\(\s*"/g) || []).length;
  }
  compare(
    'AUTOCORRECT_ENTRY_COUNT',
    fwDictCount,
    extract(
      tsFixture,
      /export const AUTOCORRECT_ENTRY_COUNT\s*=\s*(\d+)/,
      'app AUTOCORRECT_ENTRY_COUNT',
    ),
  );
}

if (drift.length) {
  console.error('kcp protocol DRIFT detected (firmware kcp.rs vs app TS client):');
  for (const d of drift) console.error('  - ' + d);
  process.exit(1);
}
console.log(
  `kcp protocol in sync — ${checked.length} constants/groups/opcodes checked, all match.`,
);
