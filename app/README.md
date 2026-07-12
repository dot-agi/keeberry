<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# keeberry configurator

The configurator for [keeberry](https://github.com/dot-agi/keeberry) keyboards.
It speaks **kcp** — keeberry's 32-byte binary protocol — over a raw-HID vendor
interface, and runs two ways from the same React/TypeScript code:

- **Web** — over the **WebHID API**, entirely in the browser, no install.
- **Native desktop** — a [Tauri](https://tauri.app) app ([`src-tauri/`](./src-tauri/))
  that talks HID through a Rust `hidapi` bridge (WebHID doesn't exist in the
  macOS webview) and additionally **flashes firmware** — enter the bootloader,
  write the bundled image, reboot, in one click.

It covers device selection, the `kcp` client library (frame codec + a transport
abstraction), and a capability-gated editor for each firmware command group:
**INFO**, **KEYMAP** (live matrix editor), **HID/KRO** (6KRO ↔ NKRO),
**TELEMETRY** (live dashboard), **CONFIG** (save to flash / load defaults /
storage info), **MACRO**, **RGB**, **BEHAVIOR** (SOCD, key overrides, tap-dance,
combos), **WIRELESS** (transport, pairing, battery, sleep) and **SYSTEM** (reboot
/ enter bootloader). The UI renders a panel only for the groups the connected
firmware advertises in its capabilities bitmask.

## Requirements

- **Web:** WebHID is only in **Chromium-based browsers** (Chrome, Edge, Opera,
  Brave) served over **HTTPS or `localhost`**. Firefox and Safari don't implement
  WebHID; the app detects this and shows a notice.
- **Native desktop:** no browser restriction, and the only way to flash firmware.
  See [`../.github/RELEASING.md`](../.github/RELEASING.md) for builds.

## Getting started

```bash
npm install
npm run dev        # web dev server — http://localhost:5173
npm run tauri:dev  # native desktop app (builds src-tauri/)
```

Click **Connect device** and pick your keyboard. The app reads the protocol
version, firmware version, chip, matrix dimensions, layer count, connection, and
the decoded capability list.

## Scripts

| Script                                      | Description                                      |
| ------------------------------------------- | ------------------------------------------------ |
| `npm run dev`                               | Vite dev server with HMR.                        |
| `npm run build`                             | Type-check (`tsc`) then build to `dist/`.        |
| `npm run preview`                           | Preview the production build locally.            |
| `npm test`                                  | Run the Vitest suite once.                       |
| `npm run lint` / `npm run format`           | ESLint / Prettier.                               |
| `npm run check:protocol`                    | Fail if the TS client has drifted from `kcp.rs`. |
| `npm run tauri:dev` / `npm run tauri:build` | Run / build the native app.                      |

## Deployment (web)

`npm run build` emits a static bundle to `dist/` (no server runtime), so it
deploys to any static host. [`vercel.json`](./vercel.json) pins the Vite preset,
the build command, the `dist` output and an SPA rewrite. As part of the keeberry
monorepo, the Vercel project sets **Root Directory = `app`** (see
[`../.github/RELEASING.md`](../.github/RELEASING.md)). Serve over **HTTPS** (or
`localhost`) — WebHID is refused on insecure origins.

## The kcp protocol

One 32-byte raw-HID report is exactly one message (no report ID):

```
request : [0]=CMD       [1]=SEQ  [2..32]=payload (30 bytes)
reply   : [0]=CMD|0x80  [1]=SEQ  [2]=STATUS  [3..32]=payload (29 bytes)
```

- The vendor interface is usage page `0xFF60`, usage `0x61`.
- The CMD high nibble selects a command group (INFO `0x0x`, KEYMAP `0x1x`, …);
  the low nibble selects the operation.
- A reply sets bit 7 of CMD and echoes the request's SEQ, so a reply pairs on
  `reply.cmd === (req.cmd | 0x80)` and `reply.seq`.
- `STATUS`: `0` OK, `1` BAD_CMD, `2` BAD_ARG, `3` BUSY, `4` UNSUPPORTED.
- The firmware advertises its groups via the INFO capabilities bitmask, so the UI
  shows only the supported ones.

The wire format is the firmware's, defined in
[`../firmware/src/kcp.rs`](../firmware/src/kcp.rs); this client mirrors it, and
`npm run check:protocol` fails the build if they diverge (see
[`../protocol/README.md`](../protocol/README.md)).

## Project structure

```
src/
  kcp/                   the kcp client library
    protocol.ts          wire constants — Status / Group / Cmd, frame offsets
    codec.ts             encode/decode 32-byte frames, SeqCounter
    bytes.ts             little-endian reads
    keycode.ts           16-bit keycode model + named catalogue
    info / keymap / hidkro / telemetry / config / macro / rgb / behavior / wireless / system
                         per-group typed parsers + encoders
    transport-iface.ts   the Transport abstraction (one seam, two backends)
    webhid-transport.ts  WebHID transport (browser)
    tauri-transport.ts   native transport (invokes the Rust hidapi bridge)
    transport.ts         KcpConnection — the SEQ-matched transact() engine
    client.ts            KcpClient — device selection + every group wrapper
    backup.ts / snapshot.ts   persist-across-flash backup + full-config snapshot
    index.ts             public API barrel
    firmware-fixture.ts  test fixture mirroring kcp.rs's reply packing
    *.test.ts            Vitest suites
  ui/                    React components (one panel per command group)
    useKcpDevice.ts      connection lifecycle + WebHID/native transport selection
    useFirmwareFlash.ts / preFlashBackup.ts / nativeFlash.ts   native flash flow
    Panel.tsx            shared Panel / Field / ErrorBanner primitives
    *Panel.tsx / *Card.tsx / KeymapEditor.tsx / KeycodePicker.tsx   the editors
  App.tsx                top-level view (+ DevicePicker, BootloaderPanel, flash banner)
  main.tsx               React entry point
src-tauri/               the native (Tauri) shell — Rust hidapi bridge + flashing
```

## `kcp` client API

```ts
import { KcpClient, WebHidTransport } from './kcp';

const transport = new WebHidTransport();
if (transport.isSupported()) {
  const client = await KcpClient.request(transport); // prompts + opens the device
  if (client) {
    const version = await client.getProtocolVersion(); // { major, minor }
    const caps = await client.getCapabilities(); // { raw, groups, present, unknownBits }
    const info = await client.getDeviceInfo(); // { chip, rows, cols, layers, connection, … }
    await client.close();
  }
}
```

Lower-level building blocks are exported too: `encodeRequest` / `decodeReply` and
`SeqCounter` (codec), the `Transport` interface, `KcpConnection` (the SEQ-matched
`transact` engine over a `Transport`-opened device), and the INFO parsers
`parseProtocolVersion`, `parseCapabilities`, and `parseDeviceInfo`.

## License

GPL-2.0-or-later, matching the keeberry firmware. See [LICENSE](./LICENSE).
