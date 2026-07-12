<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# kcp protocol contract

**kcp** is the 32-byte raw-HID protocol between the keeberry firmware and its
configurator (web + native app). It has a single source of truth: the firmware.

- **Source of truth** — the firmware:
  - `firmware/src/kcp.rs` — framing (`MSG_LEN`, `REPLY_FLAG`, the
    byte offsets), the `Status` enum, the command `group` bit indices, the
    `CAPABILITIES` mask, the command opcodes (`CMD_*`), and payload lengths
    (`DEVICE_INFO_LEN`, …).
  - `firmware/src/config.rs` — `SCHEMA_VERSION` (the on-flash config
    schema).
  - `firmware/src/usb.rs` — the raw-HID report descriptor (the kcp
    interface's usage page `0xFF60` / usage `0x61`).
- **Mirror** — the TypeScript client, kept byte-for-byte in lockstep:
  - `app/src/kcp/protocol.ts` — framing, `USAGE_PAGE`/`USAGE`, `Status`, `Group`, `Cmd`.
  - `app/src/kcp/firmware-fixture.ts` — `SCHEMA_VERSION`, `CAPABILITIES`.
  - `app/src/kcp/info.ts` — `DEVICE_INFO_LEN`.

## Drift-check

`app/scripts/check-protocol-drift.mjs` (run with `npm run check:protocol` in
`app/`, and gated in CI) extracts the shared constants from **both** sides and
exits non-zero if they diverge, so a firmware↔app mismatch is caught before it
ships. It cross-checks the framing offsets, `MSG_LEN`/`REPLY_FLAG`, the `Status`
discriminants, the command-group bit indices, the `CAPABILITIES` mask, the USB
usage page/usage, `SCHEMA_VERSION`, `DEVICE_INFO_LEN`, and a name-paired 1:1
comparison of every command opcode (so swapping two opcodes' values is caught,
which a value-set comparison would miss).

When you change the wire format, edit `kcp.rs` (and `config.rs`) **first**, then
update the TS mirror; the drift-check keeps them honest. A full codegen from a
single declarative source is a possible future step — this lightweight check is
the v1 contract.
