<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# Releasing & CI

The monorepo ships two artifacts independently, each driven by a **distinct tag
prefix**. Tag prefixes (not `paths:` filters) are used because GitHub ignores
`paths:` filters on tag pushes.

## Cut a firmware release

```sh
git tag fw-v0.1.0
git push origin fw-v0.1.0
```

`.github/workflows/firmware-release.yml` then builds `keeberry` for
`thumbv7m-none-eabi`, flattens the ELF to a raw `keeberry-<version>.bin`, **fails
if the image exceeds the flash budget** (`0x1F800` = 129024 B = 128 KiB minus the
2 KiB `CONFIG_REGION`; mirrors `firmware/memory.x`), writes a
`.sha256` sidecar, and publishes a GitHub Release with both files plus
auto-generated notes.

## Cut an app release

```sh
git tag app-v0.1.0
git push origin app-v0.1.0
```

`.github/workflows/app-release.yml` runs on `macos-latest`, builds the Tauri
desktop app, and opens a **draft** GitHub Release with the installers. Keep the
tag version in sync with `app/src-tauri/tauri.conf.json` (`tauri-action`
substitutes `__VERSION__` from it). The app's prebuild cross-compiles the
firmware from the same commit and bundles that `.bin`, which is why the release
toolchain also installs the `thumbv7m-none-eabi` target and `cargo-binutils`.

### Flasher sidecar (libusb) — required before distributing

The bundled `wb32-dfu-updater_cli` sidecar dynamically links Homebrew's
`libusb-1.0.0.dylib` by **absolute path** (`otool -L` shows
`/opt/homebrew/opt/libusb/lib/...`). That works on a dev Mac with `brew install
libusb`, but a distributed `.app` **fails to flash on a clean Mac**. Before the
first public desktop release — do this together with Developer-ID signing below:

- Make the sidecar self-contained: bundle the dylib into the `.app` and rewrite
  its load command, e.g. `dylibbundler -od -b -x
  <app>/Contents/MacOS/wb32-dfu-updater_cli -d <app>/Contents/Frameworks -p
  @executable_path/../Frameworks`, then re-sign it; **or** ship a
  statically-linked `wb32-dfu-updater_cli`.
- Add a release-CI **`otool -L` gate** that fails if any bundled binary still
  references an absolute `/opt/homebrew` (or other non-`@`-relative) path, so a
  broken sidecar can never ship silently.

Tracked in `app/src-tauri/binaries/NOTICE`.

### Signing (current vs. later)

- **Now:** **ad-hoc** signing via `APPLE_SIGNING_IDENTITY: '-'`. No Apple secrets
  required. This stops Apple-Silicon Macs from reporting the download as
  "damaged"; the user still allows it once under *Privacy & Security*.
- **Later (Developer-ID + notarization):** add repo secrets and let
  `tauri-action` import them into a temporary keychain — replace the ad-hoc
  value with `APPLE_CERTIFICATE` (base64 `.p12`), `APPLE_CERTIFICATE_PASSWORD`,
  and the real `APPLE_SIGNING_IDENTITY`; then notarize with either an App Store
  Connect API key (`APPLE_API_ISSUER` / `APPLE_API_KEY` / `APPLE_API_KEY_PATH`)
  or an Apple ID (`APPLE_ID` / `APPLE_PASSWORD` <app-specific> / `APPLE_TEAM_ID`).

### Cross-platform (later)

`app-release.yml` is macOS-only today. To also ship Windows and Linux, wrap the
`release` job in a `strategy.matrix` over `macos-latest` / `windows-latest` /
`ubuntu-latest`, and on the Linux runner apt-install the Tauri build deps
(`libwebkit2gtk-4.1-dev`, `libsoup-3.0-dev`, `librsvg2-dev`, `build-essential`,
etc.) before `tauri-action`.

## PR / push CI

`.github/workflows/ci.yml` runs on pull requests and pushes to `main`. A
`changes` job (`dorny/paths-filter`) gates two area jobs:

- **firmware-ci** (on `firmware/**`): `cargo build --release` + `cargo clippy
  -D warnings` + the flash-size gate.
- **app-ci** (on `app/**`, `macos-latest`): `npm ci` + `npm run build` + `npm
  test` + `npm run lint` + `npx prettier --check .` + `npm run check:protocol`
  (kcp protocol/codegen drift check) + `cargo check` of the Tauri shell.

An always-run **`ci-success`** aggregator passes only when every needed job
succeeded or was path-filtered out. **Require only `ci-success`** in branch
protection so filtered-out jobs don't block merges.

## Website (Vercel)

The web app deploys through **Vercel's Git integration** — not GitHub Actions.
In the Vercel project set **Root Directory = `app`** (Vercel then reads
`app/vercel.json`). To skip builds when only firmware changed, set the project's
**Ignored Build Step** to:

```sh
git diff --quiet HEAD^ HEAD -- .
```

Run from the `app` root directory, this exits `0` (→ skip the build) when the
last commit touched nothing under `app/`, and non-zero (→ build) otherwise.
