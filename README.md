<!-- SPDX-License-Identifier: GPL-2.0-or-later -->

# keeberry

Open-source firmware **and** configurator for the **Akko 5075B** (75% tri-mode
wireless) mechanical keyboard — a from-scratch Rust replacement for the stock QMK
firmware on the Westberry WB32FQ95, with a web + native app to configure it.

A monorepo:

| Path                     | What it is                                                                                                                                                                                                                                                                    |
| ------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| [`firmware/`](firmware/) | The keyboard firmware — Rust on the [embassy](https://embassy.dev) async runtime (`target thumbv7m-none-eabi`). The `pac` peripheral-access crate (WB32FQ95 register definitions, from `wb32fq95.svd`) lives in [`firmware/pac/`](firmware/pac/).                             |
| [`app/`](app/)           | The **keeberry configurator** — a React/TypeScript app speaking **kcp** (keeberry's 32-byte raw-HID protocol). Runs in the browser over WebHID, and as a native desktop app ([`app/src-tauri/`](app/src-tauri/) — Tauri + a Rust `hidapi` bridge) that also flashes firmware. |
| [`protocol/`](protocol/) | The kcp wire contract — `firmware/src/kcp.rs` is the source of truth; a [drift-check](app/scripts/check-protocol-drift.mjs) keeps the TS client in lockstep.                                                                                                                  |
| [`.github/`](.github/)   | CI + release pipelines — see [`.github/RELEASING.md`](.github/RELEASING.md).                                                                                                                                                                                                  |

## Hardware

- **MCU** — Westberry WB32FQ95 (Arm Cortex-M3 @ 96 MHz; 128 KB flash / 28 KB SRAM)
- **Radio** — WCH CH582F (BLE 5.3 / 2.4 GHz) over UART
- **Matrix** — 6 × 15, ROW2COL diodes
- **Lighting** — per-key WS2812

## Status

Runs on real hardware: typing, RGB, the kcp protocol, settings persistence, and
software DFU entry are all verified on a 5075B. Wireless is up too — BLE and
2.4 GHz typing are both hardware-validated (kcp tunnels over the 2.4 GHz link).
It's a fresh bring-up, and firmware flashing stays cable-only: there is no
wireless/OTA update path. The native app additionally does one-click on-device
firmware updates.

## Firmware Comparison

| Dimension                                    | 🦀 keeberry    | QMK            | Vial            | 🦀 RMK         | ZMK             | 🔒 Akko stock  |
| -------------------------------------------- | -------------- | -------------- | --------------- | -------------- | --------------- | -------------- |
| Language / runtime                           | Rust + embassy | C              | C (QMK fork)    | Rust + embassy | C / Zephyr      | C, closed ⁵    |
| Target board(s)                              | Akko 5075B     | 1000s          | many (QMK ⊂)    | many MCUs      | many (BLE-first)| Akko only      |
| Open source                                  | GPL-2.0        | GPL-2.0        | GPL-2.0         | MIT/Apache     | MIT             | proprietary    |
| Live GUI config, no reflash                  | ✅ web+native  | 🟡 via VIA     | ✅ headline     | ✅ via Vial    | ✅ Studio       | ✅ Cloud Drv ⁶|
| RGB effects 🌈                               | 50             | ~50            | ~50 (QMK)       | 🟡 basic       | 🟡 ~4 underglow | ✅ vendor set |
| Per-key reactive RGB                         | ✅ 10          | ✅             | ✅              | ❌             | ❌              | ✅            |
| RGB zones (per-zone effects / sync)          | ✅             | 🟡 custom      | 🟡 custom       | ❌             | ❌              | ❌             |
| Layers                                       | 16             | 16–32 ¹        | ≤16 ¹           | configurable   | configurable    | 🟡 profiles    |
| Tap-dance / combos                           | ✅             | ✅             | ✅              | ✅             | 🟡 combos       | ❌            |
| Tap-hold (Mod-Tap / Layer-Tap)               | ✅             | ✅             | ✅              | ✅             | ✅              | ❌             |
| Dynamic macros                               | ✅             | ✅             | ✅              | ✅             | ✅              | ✅            |
| On-board macro record                        | ✅             | ✅             | 🟡 ⁴            | ❌             | ❌              | ✅            |
| Autocorrect                                  | ✅             | ✅             | 🟡 ⁴            | ❌             | ❌              | ❌             |
| Unicode input                                | ✅             | ✅             | 🟡 ⁴            | ❌             | ❌              | ❌             |
| Mouse keys                                   | ✅             | ✅             | ✅              | ✅             | ✅              | ❌            |
| HID gamepad 🎮                               | ✅             | ✅             | ❌              | ❌             | ❌              | ❌            |
| **XInput (Xbox 360)** 🎮                     | **✅** ²       | ❌             | ❌              | ❌             | ❌              | ❌            |
| USB-MIDI 🎹                                  | ✅             | ✅             | 🟡 ⁴            | ❌             | ❌              | ❌            |
| HID digitizer                                | ✅             | ✅             | 🟡 ⁴            | ❌             | ❌              | ❌            |
| Wireless — BT / 2.4 GHz 🔋                   | ✅ ³           | 🟡 legacy      | ❌              | ✅ BLE         | ✅ BLE-first    | ✅ tri-mode   |
| Live tuning (debounce / auto-shift / leader) | ✅ Vial-style  | 🟡 compile-time| ✅ QMK Settings | 🟡 config file | 🟡 config/Studio| ❌            |
| SOCD / key overrides                         | ✅ both        | ✅ ⁷           | ✅ overrides    | 🟡             | 🟡              | 🟡 gaming     |
| Software DFU, no SWD probe                    | ✅             | ✅             | ✅              | 🟡 board-dep   | ✅ UF2          | 🟡 vendor tool |

<sup>¹ Layer caps are a build choice: QMK defaults to 16 (up to 32), Vial's GUI is commonly 16; keeberry is fixed at 16. ·
² XInput is first-class on Windows/Linux; macOS recognizes the controller through the OS's built-in Xbox support, not a keeberry-native macOS driver — and it's a brand-new addition. ·
³ Freshly hardware-validated: BLE and 2.4 GHz typing both work over the CH582 radio (kcp tunnels over the 2.4 GHz link). A brand-new bring-up — and firmware flashing stays cable-only; there is no wireless/OTA update path (see [Status](#status)). ·
⁴ Inherited from Vial's QMK base at compile time, not surfaced in the Vial GUI. ·
⁵ The wired 5075B *VIA* variant is QMK-based; the wireless Cloud-Driver firmware is closed and its exact base is undocumented. ·
⁶ Windows-only, closed-source. ·
⁷ QMK key overrides are core; SOCD ships as a community module, not core. Vial inherits the key overrides, not SOCD.</sup>

**Where the others lead — credit where it's due.** QMK is the giant: ~50 RGB
Matrix effects 🌈, up to 32 layers, a deep gamepad / MIDI / pointing stack, and a
board-and-feature ecosystem nothing here approaches. **Vial** owns live,
no-recompile configuration and the polished *QMK Settings* tuning panel. **ZMK**
is wireless-first 🔋 — its BLE stack and ZMK Studio are far more mature than
keeberry's freshly-validated radio. **RMK** is the other modern-Rust 🦀 firmware,
already shipping BLE and native Vial across many MCUs. And the **stock Akko**
firmware, closed as it is, still ships the most polished
tri-mode wireless _on this exact board_ today.

**Where keeberry stands out.** For something this young it's dense: 50 RGB
effects (10 of them per-key reactive) plus independently-addressable RGB zones,
the full tap-dance / combos / mod-tap / layer-tap / macros / SOCD / key-override
/ leader / auto-shift kit, a complete text stack (autocorrect and OS-native
unicode input), on-board macro **recording**, a live web + native configurator
with a Vial-style tuning panel, and probe-less software DFU. Its tri-mode radio
is up now, too: BLE and 2.4 GHz typing are both hardware-validated — though, like
the others on this board, firmware still flashes only over cable. Most
distinctively, it's the **only open firmware in this list that speaks native
XInput** 🎮 — presenting the 5075B to Windows and Linux as an Xbox 360 controller
— alongside USB-MIDI 🎹 and a HID digitizer. And unlike the factory firmware it
replaces, every byte of it is open (GPL-2.0).

## Building & flashing the firmware

Needs the `thumbv7m-none-eabi` Rust target and `cargo-binutils`:

```sh
cd firmware
cargo build -p keeberry --release
rust-objcopy -O binary target/thumbv7m-none-eabi/release/keeberry keeberry.bin
```

There is no SWD probe in the loop — flash over USB with
[`wb32-dfu-updater`](https://github.com/WestberryTech/wb32-dfu-updater) (enter the
bootloader by holding **Esc** while plugging in, or from the app):

```sh
wb32-dfu-updater_cli -s 0x08000000 -D keeberry.bin -R
```

## Configurator

See [`app/README.md`](app/README.md). The web app is a static Vite bundle
(deployed via Vercel); the native desktop app additionally does one-click
firmware updates. Both speak kcp over raw-HID.

## License

[GPL-2.0-or-later](LICENSE).
