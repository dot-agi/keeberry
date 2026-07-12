// SPDX-License-Identifier: GPL-2.0-or-later
//! Per-key RGB: a WS2812 chain driven over SPIM2, plus a small effect engine
//! (global, grid-spatial and keypress-reactive effects), an optional status-
//! indicator overlay, and the shared state the kcp RGB group ([`crate::kcp`])
//! writes.
//!
//! # Hardware (Akko 5075B donor)
//!
//! The board's per-key LEDs are a single chain of [`LED_COUNT`] WS2812s whose
//! serial data line is **PB15**, driven as the MOSI/data output of **SPIM2**
//! (the WB32 `SPIDM2` master). A separate enable pin, **PA8**
//! (`LED_POWER_EN`), gates the LED rail. These facts come from the QMK donor:
//!
//! * data pin PB15, SPI driver: `keyboard.json` `ws2812 = { driver: spi, pin:
//!   "B15" }` and `config.h:110` `WS2812_SPI_DRIVER SPIDM2`
//!   (`keyboards/akko/5075b/ansi/`).
//! * SPI clock divisor 32: `config.h:111` `WS2812_SPI_DIVISOR 32`.
//! * LED rail enable PA8: `config.h:7` `LED_POWER_EN_PIN A8`.
//! * LED count 105: the `rgb_matrix.layout` array in `keyboard.json` has 105
//!   entries, so QMK derives `RGB_MATRIX_LED_COUNT == WS2812_LED_COUNT == 105`
//!   (the donor's 22-LED custom `rgblight` is just an overlay over the last 22
//!   of these — `rgb_record/rgb_rgblight.c` — not a second physical chain).
//!
//! # WS2812-over-SPI encoding
//!
//! There is no WS2812 peripheral; the standard trick (QMK
//! `platforms/chibios/drivers/ws2812_spi.c`) is to clock out a fixed bit
//! pattern on MOSI so each WS2812 bit becomes four SPI bits at ~3 MHz:
//! a WS2812 `1` is `0b1110` (≈3/4 high) and a `0` is `0b1000` (≈1/4 high).
//! Two WS2812 bits pack into one SPI byte, so each colour byte expands to four
//! SPI bytes (MSB pair first); see [`protocol_eq`], a direct port of that
//! driver's `get_protocol_eq` (ws2812_spi.c:123-134). Colours go out in **GRB**
//! order (`WS2812_BYTE_ORDER_GRB`, the QMK default, ws2812.h:70;
//! ws2812_spi.c:139-145).
//!
//! At `WS2812_SPI_DIVISOR == 32` and the 96 MHz PCLK2 that clocks SPIM2 the SPI
//! bit clock is 96 MHz / 32 = 3.0 MHz (313 ns/bit). One WS2812 bit (4 SPI bits)
//! is then ≈1.33 µs: a `1` holds ≈1.0 µs high (spec `T1H` 900 ± 150 ns) and a
//! `0` ≈313 ns high (`T0H` 350 ± 150 ns) — both inside WS2812B tolerance, which
//! is exactly the target the QMK driver documents ("baudrate should target
//! 3.2 MHz", ws2812_spi.c:42). The frame is bracketed by a 4-byte preamble of
//! zeros and [`RESET_SIZE`] trailing zero bytes so the line idles low for the
//! WS2812 reset latch (`WS2812_TRST_US == 280` µs, ws2812.h:56;
//! `RESET_SIZE = 1000 * TRST_US / (2 * TIMING)`, ws2812_spi.c:113).
//!
//! # SPIM2 setup and transfer
//!
//! [`init`] is a polled port of the WB32 SPI HAL `spi_lld_start`
//! (ChibiOS-Contrib `os/hal/ports/WB32/LLD/SPIv1/hal_spi_lld.c`): enable the
//! SPIM2 APB2 clock, pulse its reset, set the baud divisor + slave-select +
//! `CR0`, detect the FIFO depth, clear the FIFO thresholds and interrupts, then
//! enable the core. [`spim2_send`] then shifts the frame out by polling — no DMA
//! and no interrupts. `CR0` runs the core in transmit-*and*-receive mode (as the
//! HAL does), so every transmitted byte produces a received one: the send loop
//! **must drain the receive FIFO** as it transmits, never letting transmit run
//! more than the FIFO depth ahead of receive, or the RX FIFO fills and the
//! DW-SSI core stops clocking out TX (the WS2812 frame would never complete).
//! This mirrors the RX-drain / `rxtx_gap` throttle in the HAL's interrupt
//! service routine (`spi_lld_serve_event_interrupt`, hal_spi_lld.c:77-121).
//!
//! # Executor cost (polled blocking transfer)
//!
//! One frame is `TXBUF_LEN` bytes at the 3.0 MHz bit clock ≈ 3.67 ms, and the
//! polled send is a synchronous busy-wait: it blocks the *whole* single-core
//! cooperative executor (USB, matrix, kcp) for that span — yielding mid-frame
//! is impossible because the small FIFO would underflow and corrupt the WS2812
//! stream. Two mitigations keep this within budget: [`rgb_task`] sends a frame
//! only when the rendered frame actually *changes* (a static effect sends once —
//! the WS2812s hold their latched state — costing nothing thereafter), and the
//! loop runs at [`FRAME_INTERVAL_US`] (~30 Hz) so an animating effect blocks for
//! ≈3.67 ms about 30×/s (~11 % duty within budget). The busy-wait is bounded by
//! [`SPI_SEND_TIMEOUT`]: a wedged SPI core aborts the frame and re-runs [`init`]
//! rather than spinning forever and freezing the executor. A DMA-driven transfer
//! (the donor's `spiStartSend` over the WB32 DMAC, `.await`ing completion to free
//! the executor during the shift) is out of scope until the DMAC is in the PAC.

use crate::features;
use crate::gpio::{self, Pin, Port};
use crate::wireless::{self, md::MdState, Devs};
use crate::{matrix, telemetry};
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU16, AtomicU32, Ordering};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time::{Duration, Instant, Timer};
use pac::{Peripherals, Spim2};

/// Number of WS2812 LEDs on the chain (donor `RGB_MATRIX_LED_COUNT`, derived
/// from the 105-entry `rgb_matrix.layout` in the donor `keyboard.json`).
pub const LED_COUNT: usize = 105;

// === Zone geometry (the hardware-verified LED map) =========================
//
// The single 105-LED chain partitions into three fixed zones whose disjoint
// ranges tile it exactly: the keys, then the right side strip, then the left.
// These seed the v1 zone table ([`DEFAULT_ZONES`]); a host may re-range the spare
// slots over kcp ([`set_zone_range`]). Verified on silicon.

/// First chain index of the **Keys** zone (the per-key LEDs, idx `0..=82`).
const KEYS_START: u16 = 0;
/// Length of the **Keys** zone.
const KEYS_COUNT: u16 = 83;
/// First chain index of the **Right** side strip (idx `83..=93`, bottom→top).
const RIGHT_START: u16 = 83;
/// Length of the **Right** side strip.
const RIGHT_COUNT: u16 = 11;
/// First chain index of the **Left** side strip (idx `94..=104`, top→bottom).
const LEFT_START: u16 = 94;
/// Length of the **Left** side strip.
const LEFT_COUNT: u16 = 11;
const _: () = assert!(
    (KEYS_COUNT + RIGHT_COUNT + LEFT_COUNT) as usize == LED_COUNT,
    "the v1 zones must tile the chain exactly",
);

/// `LED_POWER_EN` rail-enable output (PA8, donor `config.h:7`). Driven high to
/// power the LED chain, low to cut it when RGB is disabled.
const LED_POWER_EN: Pin = Pin::new(Port::A, 8);

/// LED boost-converter enable (PA9, donor `HS_LED_BOOSTING_PIN`, `ansi.c:201`).
/// Driven high unconditionally to raise the WS2812 supply rail — without it the
/// chain stays dark even with the data line and `LED_POWER_EN` correct.
const LED_BOOST_EN: Pin = Pin::new(Port::A, 9);

/// WS2812 serial-data pin: PB15, the SPIM2 MOSI/data output.
const WS2812_DATA_PIN: u8 = 15;

// === WS2812-over-SPI frame geometry (ws2812_spi.c:105-116) =================

/// SPI bytes emitted per WS2812 colour byte: two WS2812 bits per SPI byte, so
/// eight bits need four SPI bytes (`BYTES_FOR_LED_BYTE`, ws2812_spi.c:105).
const SPI_BYTES_PER_COLOR_BYTE: usize = 4;
/// Colour channels per LED — GRB, no white (`WS2812_CHANNELS == 3`,
/// ws2812_spi.c:109).
const CHANNELS: usize = 3;
/// SPI bytes per LED (`BYTES_FOR_LED`, ws2812_spi.c:111).
const SPI_BYTES_PER_LED: usize = SPI_BYTES_PER_COLOR_BYTE * CHANNELS;

/// Leading zero bytes that hold the data line low before the first LED
/// (`PREAMBLE_SIZE`, ws2812_spi.c:114).
const PREAMBLE_SIZE: usize = 4;

/// WS2812 reset-latch low time, microseconds (`WS2812_TRST_US`, ws2812.h:56).
const WS2812_TRST_US: usize = 280;
/// WS2812 bit window, nanoseconds (`WS2812_TIMING`, ws2812.h:32).
const WS2812_TIMING_NS: usize = 1250;
/// Trailing zero bytes that hold the line low for the reset latch
/// (`RESET_SIZE = 1000 * TRST_US / (2 * TIMING)`, ws2812_spi.c:113).
const RESET_SIZE: usize = 1000 * WS2812_TRST_US / (2 * WS2812_TIMING_NS);

/// Encoded payload bytes for all LEDs.
const DATA_SIZE: usize = SPI_BYTES_PER_LED * LED_COUNT;
/// Length of the full SPI transmit buffer: preamble + data + reset latch.
pub const TXBUF_LEN: usize = PREAMBLE_SIZE + DATA_SIZE + RESET_SIZE;

// === SPIM2 register values =================================================
// Magic values are the WB32 SPI HAL `spi_lld_start` writes (hal_spi_lld.c)
// composed from the CMSIS field constants (wb32fq95xx.h), cited per line.

/// RCC.APB2ENR: SPIM2 peripheral clock enable
/// (`RCC_APB2ENR_SPIM2EN = 0x1U << 4`, wb32fq95xx.h:4170; `rccEnableSPIM2`,
/// wb32_rcc.h:348).
const RCC_APB2ENR_SPIM2EN: u32 = 0x1 << 4;
/// RCC.APB2RSTR: SPIM2 peripheral reset
/// (`RCC_APB2RSTR_SPIM2RST = 0x1U << 4`, wb32fq95xx.h:4233; `rccResetSPIM2`,
/// wb32_rcc.h:362).
const RCC_APB2RSTR_SPIM2RST: u32 = 0x1 << 4;
/// RCC.APB1ENR: GPIOA clock enable (PA8 rail-enable)
/// (`RCC_APB1ENR_GPIOAEN = 0x1U << 5`, wb32fq95xx.h:4153).
const RCC_APB1ENR_GPIOAEN: u32 = 0x1 << 5;
/// RCC.APB1ENR: GPIOB clock enable (PB15 SPIM2 data)
/// (`RCC_APB1ENR_GPIOBEN = 0x1U << 6`, wb32fq95xx.h:4154).
const RCC_APB1ENR_GPIOBEN: u32 = 0x1 << 6;

/// SPI clock divisor written to `SPIM2.BAUDR` (DW-SSI `SCKDV`): 96 MHz / 32 =
/// 3.0 MHz bit clock (donor `WS2812_SPI_DIVISOR 32`, config.h:111;
/// `spip->spi->BAUDR = SPI_BaudRatePrescaler`, hal_spi_lld.c:267).
const SPI_BAUDR_DIV32: u32 = 32;

/// SPIM2.SER: enable slave-select 0 (`SPI_SER_SE0 = 0x1U << 0`, wb32fq95xx.h:1883;
/// `spip->spi->SER |= SPI_NSS_0`, hal_spi_lld.c:268).
const SPI_SER_SE0: u32 = 0x1 << 0;

/// SPIM2.CR0: 8-bit frames, Motorola SPI format, transmit-and-receive, SPI
/// mode 0 (CPOL = CPHA = 0). Composed exactly like `spi_lld_start`
/// (hal_spi_lld.c:296-300) from `SPI_CR0_DFS_8BITS (0x7, wb32fq95xx.h:1817) |
/// SPI_CR0_FRF_SPI (0x0<<4, :1828) | SPI_CR0_TMOD_TX_AND_RX (0x0<<8, :1836)`,
/// with the config's CPOL/CPHA both 0 — i.e. the value `0x7`.
const SPI_CR0_WS2812: u32 = 0x7;

/// SPIM2.SPIENR: core enable (`SPI_SPIENR_SPI_EN = 0x1U << 0`, wb32fq95xx.h:1874).
const SPI_SPIENR_EN: u32 = 0x1 << 0;
/// SPIM2.SPIENR: core disabled — required to write CR0/BAUDR/SER (DW-SSI).
const SPI_SPIENR_DIS: u32 = 0x0;

/// SPIM2.SR: transmit FIFO not full (`SPI_SR_TFNF = 0x1U << 1`, wb32fq95xx.h:1900).
const SPI_SR_TFNF: u32 = 0x1 << 1;
/// SPIM2.SR: receive FIFO not empty (`SPI_SR_RFNE = 0x1U << 3`, wb32fq95xx.h:1902).
const SPI_SR_RFNE: u32 = 0x1 << 3;
/// SPIM2.SR: core busy shifting (`SPI_SR_BUSY = 0x1U << 0`, wb32fq95xx.h:1899).
const SPI_SR_BUSY: u32 = 0x1 << 0;

/// SPIM2 TX/RX FIFO depth, probed once in [`init`] (port of the HAL detection,
/// hal_spi_lld.c:304-313). [`spim2_send`] keeps transmit within this many bytes
/// of receive so the RX FIFO never overflows and stalls the core. Only written
/// by `init` (before [`rgb_task`] runs) and read by the send loop.
static FIFO_DEPTH: AtomicU16 = AtomicU16::new(1);

// === GPIO field codes (2 bits per pin unless noted; STM32F4-style) =========

/// MODER alternate-function mode (`0b10`).
const MODER_ALTERNATE: u32 = 0b10;
/// Maximum output drive (CURRENT `0b11`), mirroring the USB AF-pad bring-up
/// (`crate::usb`, GPIOA.CURRENT) for clean edges on the 3 MHz SPI output.
const CURRENT_MAX: u32 = 0b11;
/// AFRH selector that routes PB15 to the SPIM2 data output: **AF5**, the
/// WB32FQ95 pin-mux value for `SPIM2_MO` on PB15 (AF0 there is a system
/// function — SWO/MCO/BOOT1 — so the SPI data never reaches the pad and the LEDs
/// stay dark). This is QMK's generic `WS2812_SPI_MOSI_PAL_MODE` default (5),
/// which the Akko donor uses; verified on silicon (the earlier AF0, copied from
/// `moky/moky67` — itself a latent bug in that board — left the chain unlit).
const WS2812_DATA_AF: u32 = 5;

/// An 8-bit-per-channel colour.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    /// Red channel.
    pub r: u8,
    /// Green channel.
    pub g: u8,
    /// Blue channel.
    pub b: u8,
}

impl Rgb {
    /// All channels off.
    pub const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };
}

/// Convert HSV (each component `0..=255`, hue wrapping the colour wheel) to
/// [`Rgb`] with integer-only arithmetic (six-sextant interpolation). With
/// `s == 0` the result is the grey `(v, v, v)`.
pub fn hsv_to_rgb(h: u8, s: u8, v: u8) -> Rgb {
    if s == 0 {
        return Rgb { r: v, g: v, b: v };
    }
    // Sextant 0..=5 and the position 0..=255 within it.
    let region = (h / 43) as u16;
    let remainder = (h as u16 - region * 43) * 6;
    let (v, s) = (v as u16, s as u16);

    let p = v * (255 - s) / 255;
    let q = v * (255 - s * remainder / 255) / 255;
    let t = v * (255 - s * (255 - remainder) / 255) / 255;

    let (r, g, b) = match region {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };
    Rgb {
        r: r as u8,
        g: g as u8,
        b: b as u8,
    }
}

/// Scale a channel by a `0..=255` factor (`255` = unchanged).
#[inline]
fn scale(value: u8, factor: u8) -> u8 {
    (value as u16 * factor as u16 / 255) as u8
}

// === Effects ===============================================================

/// Solid colour: every LED shows the configured HSV.
pub const MODE_SOLID: u8 = 0;
/// Breathing: the configured colour fades in and out.
pub const MODE_BREATHING: u8 = 1;
/// Rainbow: the hue advances over time, all LEDs in unison (the "cycle all" of
/// the donor's effect set).
pub const MODE_RAINBOW: u8 = 2;
/// Vertical hue gradient: a full colour wheel spread top-to-bottom, static.
pub const MODE_GRADIENT_UD: u8 = 3;
/// Horizontal hue gradient: a full colour wheel spread left-to-right, static.
pub const MODE_GRADIENT_LR: u8 = 4;
/// Vertical hue gradient that scrolls over time (animated [`MODE_GRADIENT_UD`]).
pub const MODE_CYCLE_UD: u8 = 5;
/// Horizontal hue gradient that scrolls over time (animated [`MODE_GRADIENT_LR`]).
pub const MODE_CYCLE_LR: u8 = 6;
/// A bright band of the base colour sweeping across the columns and wrapping.
pub const MODE_BAND: u8 = 7;
/// Pinwheel: hue set by each LED's bearing from the panel centre, rotating.
pub const MODE_PINWHEEL: u8 = 8;
/// Raindrops: scattered LEDs flash to a hue near the base and fade, independently.
pub const MODE_RAINDROPS: u8 = 9;

// Keypress-reactive effects (the per-key hit table, [`LAST_PRESS`], drives them):
// each captures the hit table per frame, and a pressed key glows then fades over
// [`reactive_fade_ms`]. They form the top contiguous mode block.

/// Solid Reactive: the whole panel rests at a dim floor of the base colour and a
/// pressed key flares to full brightness, fading back.
pub const MODE_REACTIVE: u8 = 10;
/// Reactive Wide: a pressed key blooms over its grid neighbours (Chebyshev radius
/// [`WIDE_RADIUS`]) with a distance falloff, then fades; background dark.
pub const MODE_REACTIVE_WIDE: u8 = 11;
/// Cross: a pressed key lights its whole grid row and column (a plus), fading.
pub const MODE_REACTIVE_CROSS: u8 = 12;
/// Splash: a pressed key emits a hue-shifted ring that expands across the grid as
/// it fades.
pub const MODE_SPLASH: u8 = 13;
/// Reactive Rainbow: each pressed key glows a pseudo-random hue (stable for that
/// press) then fades; background dark.
pub const MODE_REACTIVE_RAINBOW: u8 = 14;

// Procedural effect families mirroring QMK RGB Matrix (parity-and-extensibility.md
// §1.2.D "RGB (the headline QMK-leads gap)"), appended after the reactive block so ids
// 0..=14 keep their meaning. Each is one `render_*` fn + one [`RGB_EFFECTS`] entry,
// reusing the existing painter and geometry helpers — integer-only, O(LED_COUNT)/frame.

/// Band (value): a single-hue bright band sweeping left→right, its value fading with the
/// horizontal distance from the moving line (QMK `BAND_VAL`).
pub const MODE_BAND_VAL: u8 = 15;
/// Band (saturation): a single-hue band sweeping left→right, desaturating to white away
/// from the moving line (QMK `BAND_SAT`).
pub const MODE_BAND_SAT: u8 = 16;
/// Pinwheel band (value): a bright wedge rotating about the panel centre, value fading
/// with the angle from it (QMK `BAND_PINWHEEL_VAL`).
pub const MODE_BAND_PINWHEEL_VAL: u8 = 17;
/// Pinwheel band (saturation): a rotating wedge desaturating with angle
/// (QMK `BAND_PINWHEEL_SAT`).
pub const MODE_BAND_PINWHEEL_SAT: u8 = 18;
/// Spiral band (value): a rotating spiral arm (angle + radius), value fading along it
/// (QMK `BAND_SPIRAL_VAL`).
pub const MODE_BAND_SPIRAL_VAL: u8 = 19;
/// Spiral band (saturation): a rotating spiral arm desaturating along it
/// (QMK `BAND_SPIRAL_SAT`).
pub const MODE_BAND_SPIRAL_SAT: u8 = 20;
/// Cycle out-in: a radial hue cycle rippling between the panel centre and edges
/// (QMK `CYCLE_OUT_IN`).
pub const MODE_CYCLE_OUT_IN: u8 = 21;
/// Cycle out-in (dual): the radial cycle mirrored about two centres, the left and right
/// panel halves (QMK `CYCLE_OUT_IN_DUAL`).
pub const MODE_CYCLE_OUT_IN_DUAL: u8 = 22;
/// Cycle spiral: a full-hue spiral (angle + radius) rotating over time
/// (QMK `CYCLE_SPIRAL`).
pub const MODE_CYCLE_SPIRAL: u8 = 23;
/// Rainbow moving chevron: a chevron (V) hue wavefront travelling across the columns
/// (QMK `RAINBOW_MOVING_CHEVRON`).
pub const MODE_RAINBOW_MOVING_CHEVRON: u8 = 24;
/// Hue breathing: the base colour breathes in value while its hue slowly drifts
/// (QMK `HUE_BREATHING`).
pub const MODE_HUE_BREATHING: u8 = 25;
/// Hue pendulum: a horizontal hue gradient rocking left and right like a pendulum
/// (QMK `HUE_PENDULUM`).
pub const MODE_HUE_PENDULUM: u8 = 26;
/// Hue wave: a travelling triangle-wave of hue across the columns (QMK `HUE_WAVE`).
pub const MODE_HUE_WAVE: u8 = 27;
/// Dual beacon: a bright two-ended bar of the base colour sweeping around the centre
/// (QMK `DUAL_BEACON`).
pub const MODE_DUAL_BEACON: u8 = 28;
/// Rainbow beacon: a bright bar sweeping around a static rainbow ring
/// (QMK `RAINBOW_BEACON`).
pub const MODE_RAINBOW_BEACON: u8 = 29;
/// Jellybean raindrops: scattered drops in fully random hues, fading independently
/// (QMK `JELLYBEAN_RAINDROPS`).
pub const MODE_JELLYBEAN_RAINDROPS: u8 = 30;
/// Pixel rain: sparse pixels flicking on to random hues and fading, a digital-rain feel
/// (QMK `PIXEL_RAIN`).
pub const MODE_PIXEL_RAIN: u8 = 31;
/// Pixel flow: a flowing per-column hue texture drifting down the panel
/// (QMK `PIXEL_FLOW`).
pub const MODE_PIXEL_FLOW: u8 = 32;
/// Starlight: key LEDs softly twinkle in the base hue with random per-LED phase
/// (QMK `STARLIGHT`).
pub const MODE_STARLIGHT: u8 = 33;
/// Starlight (dual hue): the twinkle alternates between the base hue and its complement
/// (QMK `STARLIGHT_DUAL_HUE`).
pub const MODE_STARLIGHT_DUAL_HUE: u8 = 34;
/// Solid multisplash: expanding rings from every recent press over a dim base floor
/// (QMK `SOLID_MULTISPLASH`).
pub const MODE_SOLID_MULTISPLASH: u8 = 35;
/// Solid reactive multiwide: every recent press blooms over its neighbours on a base
/// floor (QMK `SOLID_REACTIVE_MULTIWIDE`).
pub const MODE_SOLID_REACTIVE_MULTIWIDE: u8 = 36;
/// Multinexus: every recent press lights a soft cross (row/column falloff) on a base
/// floor (QMK `SOLID_REACTIVE_MULTINEXUS`).
pub const MODE_MULTINEXUS: u8 = 37;

// A second batch of QMK RGB Matrix families, closing the headline 38→50 effect-count gap
// (parity-and-extensibility.md §1.2.D), appended after ids 0..=37 so those keep their
// meaning. Same recipe: one `render_*` fn + one [`RGB_EFFECTS`] entry each. Two are
// framebuffer effects (a small per-LED `u8` state buffer); the rest reuse the painter,
// geometry and reactive helpers above with integer-only, O(LED_COUNT)/frame math.

/// Alphas/mods: the interior letter block shows the base hue and the perimeter modifier
/// frame its complement — a static two-hue split (QMK `ALPHAS_MODS`).
pub const MODE_ALPHAS_MODS: u8 = 38;
/// Rainbow pinwheels: two mirrored rainbow arms rotating about the panel centre
/// (QMK `RAINBOW_PINWHEELS`).
pub const MODE_RAINBOW_PINWHEELS: u8 = 39;
/// Pixel fractal: a symmetric brightness wave travelling outward from the centre column
/// with a per-column hue jitter, a mirrored pixel bloom (QMK `PIXEL_FRACTAL`).
pub const MODE_PIXEL_FRACTAL: u8 = 40;
/// Riverflow: a flowing triangle wave of hue along the panel diagonal, drifting over time
/// like a current (QMK `RIVERFLOW`).
pub const MODE_RIVERFLOW: u8 = 41;
/// Typing heatmap: each keypress heats its key and the heat cools over time, mapped
/// cold-blue→hot-red — a framebuffer effect over the per-LED [`HEAT`] buffer
/// (QMK `TYPING_HEATMAP`).
pub const MODE_TYPING_HEATMAP: u8 = 42;
/// Digital rain: bright drips fall down the columns leaving fading trails — a framebuffer
/// effect over the per-LED [`RAIN`] buffer (QMK `DIGITAL_RAIN`).
pub const MODE_DIGITAL_RAIN: u8 = 43;
/// Solid reactive (simple): a pressed key flares to the base value and fades — no hue shift
/// and no floor, background dark (QMK `SOLID_REACTIVE_SIMPLE`).
pub const MODE_SOLID_REACTIVE_SIMPLE: u8 = 44;
/// Reactive nexus: a pressed key lights a soft cross (row/column falloff) on a dark
/// background — the no-floor counterpart of [`MODE_MULTINEXUS`] (QMK `SOLID_REACTIVE_NEXUS`).
pub const MODE_REACTIVE_NEXUS: u8 = 45;
/// Reactive cross (solid): the most-recent press lights a hard row/column cross over a dim
/// base floor — the single-source cross (QMK `SOLID_REACTIVE_CROSS`).
pub const MODE_SOLID_REACTIVE_CROSS: u8 = 46;
/// Reactive multicross: every recent press lights a hard cross over a dim base floor — the
/// floored counterpart of [`MODE_REACTIVE_CROSS`]'s dark background (QMK
/// `SOLID_REACTIVE_MULTICROSS`).
pub const MODE_SOLID_REACTIVE_MULTICROSS: u8 = 47;
/// Solid splash: a single expanding ring from the most-recent press over a dim base floor
/// (QMK `SOLID_SPLASH`).
pub const MODE_SOLID_SPLASH: u8 = 48;
/// Starlight (dual saturation): the twinkle alternates between full and half saturation per
/// LED (QMK `STARLIGHT_DUAL_SAT`).
pub const MODE_STARLIGHT_DUAL_SAT: u8 = 49;

/// Number of effect modes (the valid `mode_id` range is `0..MODE_COUNT`).
pub const MODE_COUNT: u8 = 50;
/// The effect-mode ids, for the kcp `LIST_MODES` reply.
pub const MODE_IDS: [u8; MODE_COUNT as usize] = [
    MODE_SOLID,
    MODE_BREATHING,
    MODE_RAINBOW,
    MODE_GRADIENT_UD,
    MODE_GRADIENT_LR,
    MODE_CYCLE_UD,
    MODE_CYCLE_LR,
    MODE_BAND,
    MODE_PINWHEEL,
    MODE_RAINDROPS,
    MODE_REACTIVE,
    MODE_REACTIVE_WIDE,
    MODE_REACTIVE_CROSS,
    MODE_SPLASH,
    MODE_REACTIVE_RAINBOW,
    MODE_BAND_VAL,
    MODE_BAND_SAT,
    MODE_BAND_PINWHEEL_VAL,
    MODE_BAND_PINWHEEL_SAT,
    MODE_BAND_SPIRAL_VAL,
    MODE_BAND_SPIRAL_SAT,
    MODE_CYCLE_OUT_IN,
    MODE_CYCLE_OUT_IN_DUAL,
    MODE_CYCLE_SPIRAL,
    MODE_RAINBOW_MOVING_CHEVRON,
    MODE_HUE_BREATHING,
    MODE_HUE_PENDULUM,
    MODE_HUE_WAVE,
    MODE_DUAL_BEACON,
    MODE_RAINBOW_BEACON,
    MODE_JELLYBEAN_RAINDROPS,
    MODE_PIXEL_RAIN,
    MODE_PIXEL_FLOW,
    MODE_STARLIGHT,
    MODE_STARLIGHT_DUAL_HUE,
    MODE_SOLID_MULTISPLASH,
    MODE_SOLID_REACTIVE_MULTIWIDE,
    MODE_MULTINEXUS,
    MODE_ALPHAS_MODS,
    MODE_RAINBOW_PINWHEELS,
    MODE_PIXEL_FRACTAL,
    MODE_RIVERFLOW,
    MODE_TYPING_HEATMAP,
    MODE_DIGITAL_RAIN,
    MODE_SOLID_REACTIVE_SIMPLE,
    MODE_REACTIVE_NEXUS,
    MODE_SOLID_REACTIVE_CROSS,
    MODE_SOLID_REACTIVE_MULTICROSS,
    MODE_SOLID_SPLASH,
    MODE_STARLIGHT_DUAL_SAT,
];

// === Effect geometry =======================================================
//
// The time/space effects (gradients, pinwheel, …) derive each LED's position from
// its chain index on a uniform grid: the 105-LED chain factors exactly as 15 × 7,
// and 15 matches the physical column count (`matrix::NUM_COLS`). Index `i` is column
// `i % GRID_W`, row `i / GRID_W` — a cheap approximation that suffices for
// whole-panel gradients. The keypress-reactive effects instead use the accurate
// donor-derived [`KEY_LED`] map and per-LED [`LED_COL`]/[`LED_ROW`] coordinates, so
// a press lights and blooms at the physical key.

/// Grid width (columns) used to map a chain index to a position.
const GRID_W: usize = 15;
/// Grid height (rows) used to map a chain index to a position.
const GRID_H: usize = 7;
const _: () = assert!(GRID_W * GRID_H == LED_COUNT, "effect grid must cover the chain exactly");

/// Breathing period in milliseconds (one full fade-in/out cycle) at the nominal
/// speed; [`effect_phase`] scales it.
const BREATH_PERIOD_MS: u64 = 4000;
/// Milliseconds between rainbow hue steps at the nominal speed; a full 256-step
/// wheel is ~7.7 s.
const RAINBOW_STEP_MS: u64 = 30;
/// Milliseconds between scroll steps of the cycling gradient effects.
const CYCLE_STEP_MS: u64 = 40;
/// Milliseconds the sweeping band dwells on each column.
const BAND_STEP_MS: u64 = 120;
/// Milliseconds between pinwheel rotation steps.
const PINWHEEL_STEP_MS: u64 = 40;
/// Full life of one raindrop generation (fade window plus dark gap).
const RAINDROP_PERIOD_MS: u64 = 1500;
/// Portion of [`RAINDROP_PERIOD_MS`] a drop is lit and fading.
const RAINDROP_FADE_MS: u64 = 700;
/// Hue jitter (mask) a raindrop adds to the base hue, keeping the colour knob live.
const RAINDROP_HUE_SPREAD: u8 = 0x3f;
/// Milliseconds per step of the sweeping bands (BAND_VAL/SAT and the pinwheel/spiral
/// bands): how fast the bright/saturated band travels along its coordinate.
const BAND_SWEEP_STEP_MS: u64 = 24;
/// Milliseconds per hue step of the spatial cycle effects (out-in, spiral, chevron).
const CYCLE_SPACE_STEP_MS: u64 = 24;
/// Milliseconds per step of the rotating beacon bar.
const BEACON_STEP_MS: u64 = 24;
/// Period (ms) of one hue-pendulum swing / hue-wave traversal at the nominal speed.
const WAVE_PERIOD_MS: u64 = 4000;
/// Full life of one pixel-rain / jellybean generation (lit-and-fade plus dark gap).
const PIXEL_PERIOD_MS: u64 = 900;
/// Portion of [`PIXEL_PERIOD_MS`] a pixel is lit and fading.
const PIXEL_FADE_MS: u64 = 500;
/// Per-pixel hash threshold gating how many LEDs pixel-rain ever lights — kept low so the
/// effect reads as scattered rain rather than a full wash.
const PIXEL_RAIN_DENSITY: u8 = 0x60;
/// Twinkle period (ms) of one starlight pulse at the nominal speed.
const STARLIGHT_PERIOD_MS: u64 = 1600;
/// Brightness falloff per grid cell along a [`reactive_nexus`] cross arm (a press's
/// row/column glow fades to dark by ~9 cells out).
const NEXUS_FALLOFF: u8 = 28;

/// Maximum emitted HSV value — the donor's panel brightness cap
/// (`rgb_matrix.max_brightness 84`, keyboard.json:266). The 105-LED chain can
/// draw heavy current, so the value handed to [`hsv_to_rgb`] is clamped to this
/// regardless of the requested colour value or master brightness, bounding the
/// peak per-channel output (and thus the LED-rail current).
const MAX_BRIGHTNESS: u8 = 84;

/// RGB render-loop period (~30 Hz). Caps how often an animating effect pays the
/// blocking SPI transfer (see the module-level "Executor cost" note).
const FRAME_INTERVAL_US: u64 = 33_333;

/// Upper bound on one polled [`spim2_send`]. A healthy frame shifts out in
/// ≈3.67 ms (see the "Executor cost" note); this ~3x cap is the deadline past
/// which a stalled SPI core (RX FIFO wedged, transmit clock gated) is assumed
/// dead. The send is then aborted and SPIM2 re-initialised — the deliberate
/// failure mode, chosen over an unbounded busy-wait that would freeze the single
/// cooperative executor (USB, matrix, kcp) forever.
const SPI_SEND_TIMEOUT: Duration = Duration::from_millis(10);

/// Scale wall-clock `now_ms` into an animation phase whose advance rate tracks
/// `speed`: [`DEFAULT_SPEED`] reproduces the nominal millisecond cadence each
/// effect constant is tuned for, `0` nearly freezes the animation and `255` runs
/// it ~2×. There is no per-frame accumulator, so changing `speed` re-bases the
/// phase — an acceptable single-frame jump on a user-driven change. `now_ms` is
/// the boot uptime, so `now_ms * (speed + 1)` cannot overflow `u64` for any
/// realistic run length.
fn effect_phase(now_ms: u64, speed: u8) -> u64 {
    now_ms * (speed as u64 + 1) / (DEFAULT_SPEED as u64 + 1)
}

/// Breathing brightness multiplier `0..=255` for animation phase `t`. A triangle
/// wave gamma-squared so the dim end dwells, giving a natural breath.
fn breath_curve(t: u64) -> u8 {
    let phase = t % BREATH_PERIOD_MS;
    let half = BREATH_PERIOD_MS / 2;
    let tri = if phase < half { phase } else { BREATH_PERIOD_MS - phase };
    let linear = (tri * 255 / half) as u8;
    scale(linear, linear)
}

/// Column (`0..GRID_W`) of chain index `i` (see the "Effect geometry" note).
#[inline]
fn grid_col(i: usize) -> u8 {
    (i % GRID_W) as u8
}
/// Row (`0..GRID_H`) of chain index `i` (see the "Effect geometry" note).
#[inline]
fn grid_row(i: usize) -> u8 {
    (i / GRID_W) as u8
}

/// Hue offset for coordinate `coord` on an axis `span` cells long: a full colour
/// wheel spread across the axis.
#[inline]
fn axis_hue(coord: u8, span: usize) -> u8 {
    (coord as u16 * 256 / span as u16) as u8
}

/// Brightness of the sweeping [`MODE_BAND`] at chain index `i`: full at the band
/// centre and halving each column away, with the distance wrapped so the band
/// loops seamlessly across the columns.
fn band_value(i: usize, base_v: u8, t: u64) -> u8 {
    let pos = ((t / BAND_STEP_MS) % GRID_W as u64) as i32;
    let raw = (grid_col(i) as i32 - pos).abs();
    let dist = raw.min(GRID_W as i32 - raw) as u32;
    let level = (255u32 >> dist.min(8)) as u8;
    scale(base_v, level)
}

/// Integer centre of the 15×7 effect grid, shared by the angular/radial effects
/// ([`grid_angle`], [`grid_radius`], [`pinwheel_hue`]).
const GRID_CX: i32 = (GRID_W as i32 - 1) / 2;
const GRID_CY: i32 = (GRID_H as i32 - 1) / 2;
/// Largest Manhattan distance from the grid centre to an edge cell — the radial span the
/// out-in / spiral effects spread a full hue wheel across.
const GRID_MAX_RADIUS: i32 = GRID_CX + GRID_CY;

/// Bearing (`0..256` = one full turn) of chain index `i` from the grid centre — the
/// angular coordinate of the pinwheel/spiral/beacon effects (no time term).
fn grid_angle(i: usize) -> u8 {
    atan2_u8(grid_row(i) as i32 - GRID_CY, grid_col(i) as i32 - GRID_CX)
}

/// Radial coordinate `0..=255` of chain index `i`: its Manhattan distance from the grid
/// centre scaled so the centre is `0` and an edge ~`255` — the radius the out-in / spiral
/// effects spread a hue wheel across.
fn grid_radius(i: usize) -> u8 {
    let d = (grid_col(i) as i32 - GRID_CX).abs() + (grid_row(i) as i32 - GRID_CY).abs();
    (d * 255 / GRID_MAX_RADIUS) as u8
}

/// Triangle wave `0..=255..=0` over a `0..=255` phase — the cheap integer stand-in for a
/// sine the breathing/pendulum/wave/starlight effects oscillate with.
fn tri8(x: u8) -> u8 {
    if x < 128 {
        x.wrapping_mul(2)
    } else {
        (255 - x).wrapping_mul(2)
    }
}

/// Pinwheel hue at chain index `i`: the LED's bearing from the panel centre plus a
/// time-driven spin, offset by the base hue `h`.
fn pinwheel_hue(i: usize, h: u8, t: u64) -> u8 {
    h.wrapping_add(grid_angle(i))
        .wrapping_add((t / PINWHEEL_STEP_MS) as u8)
}

/// Bearing of the vector `(x, y)` scaled so a full turn is `0..256` (wrapping
/// cleanly in a `u8`), by the standard division-based integer atan2: `+x` → 0,
/// `+y` → 64, `-x` → 128, `-y` → 192. No floating point.
fn atan2_u8(y: i32, x: i32) -> u8 {
    if x == 0 && y == 0 {
        return 0;
    }
    let abs_y = y.unsigned_abs() as i32 + 1; // bias avoids a zero divisor on the axes
    // Eighth-turn = 32; the first-/second-quadrant base angles for the upper half.
    let angle = if x >= 0 {
        32 - 32 * (x - abs_y) / (x + abs_y)
    } else {
        96 - 32 * (x + abs_y) / (abs_y - x)
    };
    if y < 0 {
        (256 - angle) as u8
    } else {
        angle as u8
    }
}

/// Small integer hash (Knuth multiplicative + xorshift) → a `u8`. Deterministic,
/// so [`MODE_RAINDROPS`] needs no RNG state: each LED's schedule and hue follow
/// from its index and generation.
fn hash_u8(mut x: u32) -> u8 {
    x = x.wrapping_mul(2_654_435_761);
    x ^= x >> 15;
    x = x.wrapping_mul(2_246_822_519);
    (x >> 24) as u8
}

/// The shared raindrop envelope at chain index `i`: this LED's current drop `value`
/// (faded) and its `gen`eration index (which re-randomises a drop's colour). A per-LED
/// phase offset scatters the drops in time; the value fades across [`RAINDROP_FADE_MS`]
/// then stays dark until the next generation. Used by both the base-hued [`raindrop`] and
/// the fully-random [`render_jellybean_raindrops`].
fn raindrop_envelope(i: usize, base_v: u8, t: u64) -> (u8, u32) {
    let off = hash_u8(i as u32) as u64 * RAINDROP_PERIOD_MS / 256;
    let local = (t + off) % RAINDROP_PERIOD_MS;
    let gen = (t + off) / RAINDROP_PERIOD_MS;
    let value = if local < RAINDROP_FADE_MS {
        scale(base_v, (255 - local * 255 / RAINDROP_FADE_MS) as u8)
    } else {
        0
    };
    (value, gen as u32)
}

/// Raindrop `(hue, value)` at chain index `i`: the [`raindrop_envelope`] value with a hue
/// jittered around the base `h` (re-randomised each generation).
fn raindrop(i: usize, h: u8, base_v: u8, t: u64) -> (u8, u8) {
    let (value, gen) = raindrop_envelope(i, base_v, t);
    let hue = h.wrapping_add(hash_u8(i as u32 ^ gen) & RAINDROP_HUE_SPREAD);
    (hue, value)
}

// === Per-key reactive effects =============================================
//
// A reactive effect lights the key the user just pressed and fades it out. The
// matrix scan ([`crate::matrix`]) feeds press edges in through [`note_key_press`],
// which stamps the pressed key's LED in [`LAST_PRESS`]; the reactive effects read
// that table each frame and decay the glow. A matrix `(row, col)` maps to its chain
// index through the donor-derived [`KEY_LED`] table (the physical chain is not
// row-major), and the spatial reactive modes measure distance with the real per-LED
// [`LED_COL`]/[`LED_ROW`] coordinates. The keyless tail LEDs carry the status
// indicators instead (see [`apply_indicators`]).

/// [`LAST_PRESS`] sentinel for a key never pressed since boot (so its glow stays
/// off). A real press stamp is the boot-uptime millisecond truncated to `u32`,
/// which does not collide with this value in practice.
const NEVER_PRESSED: u32 = u32::MAX;

/// Per-LED millisecond stamp ([`Instant::now`] uptime truncated to `u32`) of the
/// last press of the key at that LED — the reactive hit table. Written by
/// [`note_key_press`] from the matrix scan and read by the reactive effects;
/// single-core
/// `Relaxed` access suffices, as for the rest of this module's shared state. Tail
/// LEDs with no matrix key stay [`NEVER_PRESSED`].
static LAST_PRESS: [AtomicU32; LED_COUNT] = [const { AtomicU32::new(NEVER_PRESSED) }; LED_COUNT];

/// Per-key typing-heatmap accumulator: [`note_key_press`] adds [`HEAT_PER_PRESS`] on each
/// press and [`render_typing_heatmap`] cools it over time, so a rapidly-typed key reads hot.
/// Cross-task like [`LAST_PRESS`] (matrix writes, the RGB task reads), so single-core
/// `Relaxed` access suffices.
static HEAT: [AtomicU8; LED_COUNT] = [const { AtomicU8::new(0) }; LED_COUNT];
/// Boot-uptime millisecond of the last [`HEAT`] cool step, so the heatmap decay tracks
/// elapsed wall-clock time rather than frame count (and stays idempotent within one frame).
static HEAT_LAST_MS: AtomicU32 = AtomicU32::new(0);
/// Heat added to a key's [`HEAT`] cell on each press (saturating).
const HEAT_PER_PRESS: u8 = 60;
/// Milliseconds of elapsed time that sheds one unit of [`HEAT`] — the heatmap cool rate.
const HEAT_COOL_MS: u32 = 32;

/// [`KEY_LED`] sentinel for a matrix position with no LED under it.
const NO_LED: u8 = 255;

/// `matrix(row, col)` → WS2812 chain index ([`NO_LED`] = no LED under that key),
/// from the donor `rgb_matrix.layout`. The physical chain is **not** row-major, so
/// the previous `row * GRID_W + col` stamped the wrong LED — the reactive effects
/// then lit and spread from the wrong place (basic Reactive's [`REACTIVE_FLOOR`] hid
/// it, but the dark-background modes showed it as "broken"). Indexed `[row][col]`.
const KEY_LED: [[u8; matrix::NUM_COLS]; matrix::NUM_ROWS] = [
    [14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, NO_LED],
    [15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29],
    [44, 43, 42, 41, 40, 39, 38, 37, 36, 35, 34, 33, 32, 31, 30],
    [45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, NO_LED, 57, 58],
    [72, NO_LED, 71, 70, 69, 68, 67, 66, 65, 64, 63, 62, 61, 60, 59],
    [73, 74, 75, NO_LED, NO_LED, NO_LED, 76, NO_LED, NO_LED, 77, 78, 79, 80, 81, 82],
];

/// Per-LED real grid column (`0..=14`) from the donor x coordinates, so the reactive
/// spread (Wide/Cross/Splash) measures distance in the true key layout rather than
/// the render grid's row-major approximation. Non-key tail LEDs rest at 0.
const LED_COL: [u8; LED_COUNT] = [0, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 14, 13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 0, 0, 1, 2, 6, 7, 10, 11, 12, 13, 14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
/// Per-LED real grid row (`0..=5`) from the donor y coordinates (see [`LED_COL`]).
const LED_ROW: [u8; LED_COUNT] = [5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 5, 5, 5, 5, 5, 5, 5, 5, 5, 5, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

/// Record a debounced key-press edge at matrix `(row, col)` into the reactive hit
/// table. Called from [`crate::matrix::Debouncer::update`] on the press edge (the
/// minimal matrix hook). Maps the matrix position to its real WS2812 chain index via
/// [`KEY_LED`], so the lit LED is the one physically under the pressed key; a
/// position with no LED ([`NO_LED`]) or out-of-range coordinates are ignored.
pub fn note_key_press(row: usize, col: usize) {
    if row >= matrix::NUM_ROWS || col >= matrix::NUM_COLS {
        return;
    }
    let led = KEY_LED[row][col];
    if led == NO_LED {
        return;
    }
    LAST_PRESS[led as usize].store(Instant::now().as_millis() as u32, Ordering::Relaxed);
    // Accumulate typing-heatmap heat at the pressed key (saturating); the heatmap effect
    // cools it over time. Two relaxed atomics — cheap enough to run on every press edge.
    let heat = &HEAT[led as usize];
    heat.store(heat.load(Ordering::Relaxed).saturating_add(HEAT_PER_PRESS), Ordering::Relaxed);
}

/// Shortest reactive fade window (ms), at the maximum [`SPEED`] (snappiest decay).
const REACTIVE_FADE_MIN_MS: u32 = 250;
/// Longest reactive fade window (ms), at [`SPEED`] 0 (most lingering decay).
const REACTIVE_FADE_MAX_MS: u32 = 1200;

/// Reactive glow lifetime for `speed`: a higher speed shortens it (a press decays
/// faster), interpolating [`REACTIVE_FADE_MAX_MS`] (speed 0) down to
/// [`REACTIVE_FADE_MIN_MS`] (speed 255). At [`DEFAULT_SPEED`] this is ~720 ms,
/// inside the ~0.5–1 s target.
fn reactive_fade_ms(speed: u8) -> u32 {
    let span = REACTIVE_FADE_MAX_MS - REACTIVE_FADE_MIN_MS;
    REACTIVE_FADE_MAX_MS - span * speed as u32 / 255
}

/// Glow intensity `0..=255` of a key last pressed at `stamp`, given the current
/// uptime `now` (ms, `u32`) and the `fade` window: full immediately after the
/// press, falling linearly to zero across `fade`, and zero for a key never pressed
/// or whose glow has expired. Wrapping subtraction tolerates the ~49-day `u32`
/// millisecond rollover.
fn reactive_intensity(stamp: u32, now: u32, fade: u32) -> u8 {
    if stamp == NEVER_PRESSED {
        return 0;
    }
    let elapsed = now.wrapping_sub(stamp);
    if elapsed >= fade {
        0
    } else {
        (255 - elapsed * 255 / fade) as u8
    }
}

/// Resting brightness floor of [`MODE_REACTIVE`] (fraction of the base value the
/// idle panel holds, so unpressed keys glow softly rather than going fully dark).
const REACTIVE_FLOOR: u8 = 0x28;
/// Chebyshev grid radius a [`MODE_REACTIVE_WIDE`] press blooms over.
const WIDE_RADIUS: u8 = 2;
/// Manhattan grid distance a [`MODE_SPLASH`] ring travels over its fade window.
const SPLASH_MAX_RADIUS: u32 = 18;
/// Half-width (grid cells) of the [`MODE_SPLASH`] expanding ring.
const SPLASH_RING_WIDTH: u32 = 2;
/// Hue shift per Manhattan cell of a [`MODE_SPLASH`] ring, so it reads as a colour
/// wavefront rather than a flat band.
const SPLASH_HUE_SPREAD: u8 = 12;

/// One frame's reactive inputs, assembled once per render ([`ReactiveFrame::capture`])
/// so the effect helpers share the work: the [`LAST_PRESS`] snapshot, the per-source
/// glow derived from it, and the timing the [`MODE_SPLASH`] wavefront needs.
struct ReactiveFrame {
    /// Per-LED last-press stamp (the [`LAST_PRESS`] snapshot).
    press: [u32; LED_COUNT],
    /// Per-source glow `0..=255` from [`reactive_intensity`], precomputed so the
    /// spatial modes' inner loops are a cheap array read.
    src: [u8; LED_COUNT],
    /// Current uptime (ms, `u32`); the splash ring's radius reference.
    now: u32,
    /// Reactive fade window (ms) for the active speed.
    fade: u32,
}

impl ReactiveFrame {
    /// Snapshot [`LAST_PRESS`] and derive each source's glow for the `fade` window
    /// at uptime `now`.
    fn capture(now: u32, fade: u32) -> Self {
        let mut press = [NEVER_PRESSED; LED_COUNT];
        let mut src = [0u8; LED_COUNT];
        for ((slot, glow), cell) in press.iter_mut().zip(src.iter_mut()).zip(LAST_PRESS.iter()) {
            *slot = cell.load(Ordering::Relaxed);
            *glow = reactive_intensity(*slot, now, fade);
        }
        Self {
            press,
            src,
            now,
            fade,
        }
    }

    /// Chain index of the most-recently pressed key still glowing (the smallest elapsed
    /// since its press among the lit sources), or `None` when nothing glows — the single
    /// source the non-"multi" reactive effects ([`render_solid_reactive_cross`],
    /// [`render_solid_splash`]) ripple from. Elapsed is compared rather than the raw stamp
    /// because every glowing press is within the sub-second fade window, so none has wrapped.
    fn latest(&self) -> Option<usize> {
        let mut best: Option<(usize, u32)> = None;
        for (j, &glow) in self.src.iter().enumerate() {
            if glow == 0 {
                continue;
            }
            let elapsed = self.now.wrapping_sub(self.press[j]);
            if best.is_none_or(|(_, e)| elapsed < e) {
                best = Some((j, elapsed));
            }
        }
        best.map(|(j, _)| j)
    }
}

/// [`MODE_REACTIVE`] `(hue, value)` for LED `i`: the base hue at a brightness that
/// rests at the dim [`REACTIVE_FLOOR`] and rises to the full base value just after
/// this key's press, then fades back.
fn reactive_solid(i: usize, h: u8, base_v: u8, frame: &ReactiveFrame) -> (u8, u8) {
    let floor = scale(base_v, REACTIVE_FLOOR);
    (h, scale(base_v, frame.src[i]).max(floor))
}

/// Whether chain LED `i` sits under a physical key. The real keys are the
/// contiguous range `1..KEYS_COUNT` (= `1..=82`); chain index 0 and the side/tail
/// LEDs (`RIGHT_START..`) have no key and only placeholder donor coordinates `(0, 0)`,
/// so the dark-background spatial reactive modes must skip them — otherwise a top-row
/// or left-column press would light them as phantom top-left neighbours.
fn is_key_led(i: usize) -> bool {
    (1..KEYS_COUNT as usize).contains(&i)
}

/// [`MODE_REACTIVE_WIDE`] `(hue, value)` for LED `i`: the strongest nearby press
/// glow, attenuated by Chebyshev distance out to [`WIDE_RADIUS`]; background dark.
fn reactive_wide(i: usize, h: u8, base_v: u8, frame: &ReactiveFrame) -> (u8, u8) {
    if !is_key_led(i) {
        return (h, 0);
    }
    let (ci, ri) = (LED_COL[i] as i32, LED_ROW[i] as i32);
    let mut best = 0u8;
    for (j, &glow) in frame.src.iter().enumerate() {
        if glow == 0 {
            continue;
        }
        let dist = (LED_COL[j] as i32 - ci)
            .abs()
            .max((LED_ROW[j] as i32 - ri).abs()) as u8;
        if dist <= WIDE_RADIUS {
            let atten = (255 - dist as u16 * 255 / (WIDE_RADIUS as u16 + 1)) as u8;
            best = best.max(scale(glow, atten));
        }
    }
    (h, scale(base_v, best))
}

/// [`MODE_REACTIVE_CROSS`] `(hue, value)` for LED `i`: lit when a pressed key
/// shares its grid row or column, at that press's glow; background dark.
fn reactive_cross(i: usize, h: u8, base_v: u8, frame: &ReactiveFrame) -> (u8, u8) {
    if !is_key_led(i) {
        return (h, 0);
    }
    let (ci, ri) = (LED_COL[i], LED_ROW[i]);
    let mut best = 0u8;
    for (j, &glow) in frame.src.iter().enumerate() {
        if glow != 0 && (LED_COL[j] == ci || LED_ROW[j] == ri) {
            best = best.max(glow);
        }
    }
    (h, scale(base_v, best))
}

/// Hard-cross glow `0..=255` at LED `i` from a single press source `src`: `src`'s glow when
/// `i` shares its grid row or column, else `0` — the single-source form of [`reactive_cross`]
/// used by [`render_solid_reactive_cross`]. Only lights real key LEDs.
fn cross_single(i: usize, src: Option<usize>, frame: &ReactiveFrame) -> u8 {
    match src {
        Some(j) if is_key_led(i) && (LED_COL[j] == LED_COL[i] || LED_ROW[j] == LED_ROW[i]) => {
            frame.src[j]
        }
        _ => 0,
    }
}

/// The [`MODE_SPLASH`] ring `(hue, value)` contributed at LED `i` by a single press source
/// `j`: an expanding ring whose radius grows with `j`'s press age across
/// [`SPLASH_MAX_RADIUS`], the band softened by [`SPLASH_RING_WIDTH`] and the hue shifted by
/// distance ([`SPLASH_HUE_SPREAD`]); `value` is `0` when `j`'s wavefront is not passing `i`.
/// The shared body of the splash effects — the looping [`reactive_splash`] (every press) and
/// the single-source [`render_solid_splash`]. `value` is `j`'s glow scaled by the ring band;
/// the caller folds in the base value.
fn splash_ring(i: usize, j: usize, h: u8, frame: &ReactiveFrame) -> (u8, u8) {
    let (ci, ri) = (LED_COL[i] as i32, LED_ROW[i] as i32);
    let dist = ((LED_COL[j] as i32 - ci).abs() + (LED_ROW[j] as i32 - ri).abs()) as u32;
    let radius = frame.now.wrapping_sub(frame.press[j]) * SPLASH_MAX_RADIUS / frame.fade;
    let delta = radius.abs_diff(dist);
    if delta <= SPLASH_RING_WIDTH {
        let band = (255 - delta * 255 / (SPLASH_RING_WIDTH + 1)) as u8;
        let hue = h.wrapping_add((dist as u8).wrapping_mul(SPLASH_HUE_SPREAD));
        (hue, scale(frame.src[j], band))
    } else {
        (h, 0)
    }
}

/// [`MODE_SPLASH`] `(hue, value)` for LED `i`: the brightest expanding [`splash_ring`] from
/// any recent press whose wavefront currently passes this cell; background dark.
fn reactive_splash(i: usize, h: u8, base_v: u8, frame: &ReactiveFrame) -> (u8, u8) {
    if !is_key_led(i) {
        return (h, 0);
    }
    let mut best_v = 0u8;
    let mut best_hue = h;
    for (j, &glow) in frame.src.iter().enumerate() {
        if glow == 0 {
            continue;
        }
        let (hue, v) = splash_ring(i, j, h, frame);
        if v > best_v {
            best_v = v;
            best_hue = hue;
        }
    }
    (best_hue, scale(base_v, best_v))
}

/// [`MODE_REACTIVE_RAINBOW`] `(hue, value)` for LED `i`: a per-press pseudo-random
/// hue — stable while the press glows, since it is hashed from the fixed press
/// stamp — at the fading glow; background dark.
fn reactive_rainbow(i: usize, base_v: u8, frame: &ReactiveFrame) -> (u8, u8) {
    let glow = frame.src[i];
    if glow == 0 {
        (0, 0)
    } else {
        (hash_u8(frame.press[i]), scale(base_v, glow))
    }
}

/// Nexus glow `0..=255` at LED `i` from the strongest recent press sharing its grid row or
/// column, the cross arm fading with distance along it ([`NEXUS_FALLOFF`] per cell) —
/// softer than the hard [`reactive_cross`] plus. Background-agnostic (the caller adds any
/// floor); like the other spatial reactive helpers it only lights real key LEDs.
fn reactive_nexus(i: usize, frame: &ReactiveFrame) -> u8 {
    if !is_key_led(i) {
        return 0;
    }
    let (ci, ri) = (LED_COL[i] as i32, LED_ROW[i] as i32);
    let mut best = 0u8;
    for (j, &glow) in frame.src.iter().enumerate() {
        if glow == 0 {
            continue;
        }
        let (dx, dy) = ((LED_COL[j] as i32 - ci).abs(), (LED_ROW[j] as i32 - ri).abs());
        if dx == 0 || dy == 0 {
            let atten = 255u16.saturating_sub((dx + dy) as u16 * NEXUS_FALLOFF as u16) as u8;
            best = best.max(scale(glow, atten));
        }
    }
    best
}

// ===========================================================================
// Effect registry
// ===========================================================================
//
// Each effect is one `render_*` fn keyed by its `MODE_*` id; the `rgb_task` frame
// loop indexes [`RGB_EFFECTS`] by the live mode to render it. Adding an effect is
// one `render_*` fn + one `RGB_EFFECTS` entry + one `MODE_*` id — no central dispatch
// to edit — so this is where the effect count grows. The math is the same per-LED
// `(hue, value)` each former match arm produced, fed through [`paint`]'s shared
// saturation + brightness clamp, so the registry is byte-for-byte the prior renderer.

/// Read-only per-frame context handed to each effect renderer in [`RGB_EFFECTS`].
///
/// `base_v` is the effect value already gated by the master brightness knob (the
/// single brightness fold), `t` the animation phase for the time/space effects
/// (from [`effect_phase`]), and `now_ms`/`speed` what the reactive effects need to
/// capture and fade a [`ReactiveFrame`].
pub struct RgbCtx {
    /// Base hue.
    pub h: u8,
    /// Base saturation.
    pub s: u8,
    /// Effect value, already scaled by the master brightness.
    pub base_v: u8,
    /// Animation speed knob (drives the reactive fade lifetime).
    pub speed: u8,
    /// Boot-uptime milliseconds of this frame (reactive frame timestamp).
    pub now_ms: u64,
    /// Animation phase for the time/space effects (from [`effect_phase`]).
    pub t: u64,
}

/// Paint each LED from a per-LED `(hue, sat, value)` source, clamping the value to
/// [`MAX_BRIGHTNESS`] (the LED-rail current cap). This is the one fold every effect
/// shares; the per-LED saturation lets the band-saturation effects desaturate to white.
/// Master brightness is already folded into the value the caller supplies, so this single
/// clamp fully bounds the output.
fn paint_full(leds: &mut [Rgb; LED_COUNT], color: impl Fn(usize) -> (u8, u8, u8)) {
    for (i, led) in leds.iter_mut().enumerate() {
        let (hue, sat, value) = color(i);
        *led = hsv_to_rgb(hue, sat, value.min(MAX_BRIGHTNESS));
    }
}

/// Paint each LED from a per-LED `(hue, value)` source at the shared base saturation
/// (`ctx.s`) — the common case, a thin wrapper over [`paint_full`] so each effect that
/// does not vary saturation supplies only its colour.
fn paint(ctx: &RgbCtx, leds: &mut [Rgb; LED_COUNT], color: impl Fn(usize) -> (u8, u8)) {
    paint_full(leds, |i| {
        let (hue, value) = color(i);
        (hue, ctx.s, value)
    });
}

/// Solid colour: every LED shows the base HSV.
fn render_solid(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |_i| (c.h, c.base_v));
}

/// Breathing: the base colour fades in and out.
fn render_breathing(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |_i| (c.h, scale(c.base_v, breath_curve(c.t))));
}

/// Rainbow: the hue advances over time, all LEDs in unison.
fn render_rainbow(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |_i| ((c.t / RAINBOW_STEP_MS) as u8, c.base_v));
}

/// Vertical hue gradient, static.
fn render_gradient_ud(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| {
        (c.h.wrapping_add(axis_hue(grid_row(i), GRID_H)), c.base_v)
    });
}

/// Horizontal hue gradient, static.
fn render_gradient_lr(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| {
        (c.h.wrapping_add(axis_hue(grid_col(i), GRID_W)), c.base_v)
    });
}

/// Vertical hue gradient that scrolls over time.
fn render_cycle_ud(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let scroll = (c.t / CYCLE_STEP_MS) as u8;
    paint(c, leds, |i| {
        (
            c.h.wrapping_add(axis_hue(grid_row(i), GRID_H)).wrapping_add(scroll),
            c.base_v,
        )
    });
}

/// Horizontal hue gradient that scrolls over time.
fn render_cycle_lr(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let scroll = (c.t / CYCLE_STEP_MS) as u8;
    paint(c, leds, |i| {
        (
            c.h.wrapping_add(axis_hue(grid_col(i), GRID_W)).wrapping_add(scroll),
            c.base_v,
        )
    });
}

/// A bright band of the base colour sweeping across the columns.
fn render_band(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| (c.h, band_value(i, c.base_v, c.t)));
}

/// Pinwheel: hue set by each LED's bearing from the panel centre, rotating.
fn render_pinwheel(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| (pinwheel_hue(i, c.h, c.t), c.base_v));
}

/// Raindrops: scattered LEDs flash near the base hue and fade independently.
fn render_raindrops(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| raindrop(i, c.h, c.base_v, c.t));
}

/// Solid Reactive: a dim base floor; a pressed key flares and fades back.
fn render_reactive(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| reactive_solid(i, c.h, c.base_v, &frame));
}

/// Reactive Wide: a pressed key blooms over its grid neighbours, then fades.
fn render_reactive_wide(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| reactive_wide(i, c.h, c.base_v, &frame));
}

/// Cross: a pressed key lights its whole grid row and column, fading.
fn render_reactive_cross(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| reactive_cross(i, c.h, c.base_v, &frame));
}

/// Splash: a pressed key emits an expanding hue-shifted ring as it fades.
fn render_splash(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| reactive_splash(i, c.h, c.base_v, &frame));
}

/// Reactive Rainbow: each pressed key glows a per-press pseudo-random hue, fading.
fn render_reactive_rainbow(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| reactive_rainbow(i, c.base_v, &frame));
}

// === Procedural effect families (parity-and-extensibility.md §1.2.D) ========
//
// QMK-RGB-Matrix-style animations appended after the original 0..=14 modes. Each reuses
// the shared painter ([`paint`]/[`paint_full`]) and the geometry helpers above, derives
// its motion from `c.t` (so the speed knob scales it) and stays O(LED_COUNT) per frame.
// Per-pixel randomness is [`hash_u8`] of the LED index, never an RNG, exactly as
// [`raindrop`] does.

/// Shared body of the sweeping value bands: the base value fades with a sawtooth of the
/// LED's `coord`inate past a moving line, so one bright band travels along that
/// coordinate. The variants differ only in the coordinate (column, angle, angle+radius).
fn band_sweep_val(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT], coord: impl Fn(usize) -> u8) {
    let phase = (c.t / BAND_SWEEP_STEP_MS) as u8;
    paint(c, leds, |i| (c.h, scale(c.base_v, coord(i).wrapping_sub(phase))));
}

/// Shared body of the sweeping saturation bands: like [`band_sweep_val`] but the band
/// desaturates the base hue to white away from the moving line (value held at base).
fn band_sweep_sat(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT], coord: impl Fn(usize) -> u8) {
    let phase = (c.t / BAND_SWEEP_STEP_MS) as u8;
    paint_full(leds, |i| (c.h, scale(c.s, coord(i).wrapping_sub(phase)), c.base_v));
}

/// Band (value): the value band sweeping across the columns.
fn render_band_val(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    band_sweep_val(c, leds, |i| axis_hue(grid_col(i), GRID_W));
}

/// Band (saturation): the saturation band sweeping across the columns.
fn render_band_sat(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    band_sweep_sat(c, leds, |i| axis_hue(grid_col(i), GRID_W));
}

/// Pinwheel band (value): the value band sweeping around the panel centre by angle.
fn render_band_pinwheel_val(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    band_sweep_val(c, leds, grid_angle);
}

/// Pinwheel band (saturation): the saturation band sweeping around the centre by angle.
fn render_band_pinwheel_sat(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    band_sweep_sat(c, leds, grid_angle);
}

/// Spiral band (value): the value band sweeping along a spiral (angle + radius).
fn render_band_spiral_val(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    band_sweep_val(c, leds, |i| grid_angle(i).wrapping_add(grid_radius(i)));
}

/// Spiral band (saturation): the saturation band sweeping along a spiral (angle + radius).
fn render_band_spiral_sat(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    band_sweep_sat(c, leds, |i| grid_angle(i).wrapping_add(grid_radius(i)));
}

/// Cycle out-in: a radial hue ripple from the panel centre, animated over time.
fn render_cycle_out_in(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        (c.h.wrapping_add(grid_radius(i)).wrapping_sub(phase), c.base_v)
    });
}

/// Cycle out-in (dual): the radial ripple measured from the nearer of two centres (the
/// left and right panel halves), so the colour ripples symmetrically inward on each side.
fn render_cycle_out_in_dual(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let row = (grid_row(i) as i32 - GRID_CY).abs();
        let col = grid_col(i) as i32;
        let dl = (col - GRID_W as i32 / 4).abs() + row;
        let dr = (col - 3 * GRID_W as i32 / 4).abs() + row;
        let radius = (dl.min(dr) * 255 / GRID_MAX_RADIUS) as u8;
        (c.h.wrapping_add(radius).wrapping_sub(phase), c.base_v)
    });
}

/// Cycle spiral: a full-hue spiral (angle + radius) rotating over time.
fn render_cycle_spiral(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let hue = c
            .h
            .wrapping_add(grid_angle(i))
            .wrapping_add(grid_radius(i))
            .wrapping_sub(phase);
        (hue, c.base_v)
    });
}

/// Rainbow moving chevron: a V-shaped hue wavefront (column offset by the row's distance
/// from the centre) travelling across the columns.
fn render_rainbow_moving_chevron(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let chevron = grid_col(i) + (grid_row(i) as i32 - GRID_CY).unsigned_abs() as u8;
        (c.h.wrapping_add(chevron.wrapping_mul(12)).wrapping_add(phase), c.base_v)
    });
}

/// Hue breathing: the base value breathes while the hue slowly drifts, so each breath
/// returns on a new colour.
fn render_hue_breathing(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let drift = (c.t / RAINBOW_STEP_MS) as u8;
    paint(c, leds, |_i| {
        (c.h.wrapping_add(drift), scale(c.base_v, breath_curve(c.t)))
    });
}

/// Hue pendulum: a horizontal hue gradient whose offset swings back and forth (a triangle
/// wave) rather than scrolling in one direction.
fn render_hue_pendulum(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let swing = tri8((c.t % WAVE_PERIOD_MS * 255 / WAVE_PERIOD_MS) as u8);
    paint(c, leds, |i| {
        let hue = c.h.wrapping_add(axis_hue(grid_col(i), GRID_W)).wrapping_add(swing);
        (hue, c.base_v)
    });
}

/// Hue wave: a travelling triangle wave of hue across the columns — colour rises then
/// falls across the panel in mirrored bands that drift over time.
fn render_hue_wave(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let wave = tri8(axis_hue(grid_col(i), GRID_W).wrapping_add(phase));
        (c.h.wrapping_add(wave), c.base_v)
    });
}

/// Brightness `0..=255` of a rotating dual beacon bar at chain index `i`: full along the
/// bar's current bearing `beam` (both ends, by folding the angle to a half-turn) and dark
/// perpendicular to it.
fn beacon_value(i: usize, beam: u8) -> u8 {
    let diff = grid_angle(i).wrapping_sub(beam) & 0x7f; // 0 and 128 (both ends) -> 0
    let dist = if diff < 64 { diff } else { 128 - diff }; // 0..=64 from the bar
    255u16.saturating_sub(dist as u16 * 4) as u8
}

/// Dual beacon: a bright two-ended bar of the base colour sweeping around the centre.
fn render_dual_beacon(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let beam = (c.t / BEACON_STEP_MS) as u8;
    paint(c, leds, |i| (c.h, scale(c.base_v, beacon_value(i, beam))));
}

/// Rainbow beacon: the beacon bar sweeps around a static rainbow ring (hue set by each
/// LED's bearing from the centre).
fn render_rainbow_beacon(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let beam = (c.t / BEACON_STEP_MS) as u8;
    paint(c, leds, |i| {
        (c.h.wrapping_add(grid_angle(i)), scale(c.base_v, beacon_value(i, beam)))
    });
}

/// Jellybean raindrops: the [`raindrop_envelope`] scatter with a fully random hue per
/// drop (a whole wheel) rather than jittered around the base.
fn render_jellybean_raindrops(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| {
        let (value, gen) = raindrop_envelope(i, c.base_v, c.t);
        (hash_u8((i as u32) ^ gen ^ 0x5a5a_5a5a), value)
    });
}

/// Pixel rain: sparse pixels flick on to a random hue and fade — only the LEDs whose
/// per-generation hash clears [`PIXEL_RAIN_DENSITY`] ever light, so it reads as scattered
/// rain rather than a full wash.
fn render_pixel_rain(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| {
        let off = hash_u8((i as u32) ^ 0x1234) as u64 * PIXEL_PERIOD_MS / 256;
        let local = (c.t + off) % PIXEL_PERIOD_MS;
        let gen = ((c.t + off) / PIXEL_PERIOD_MS) as u32;
        let lit = hash_u8((i as u32) ^ gen.wrapping_mul(0x9e37_79b1));
        if lit < PIXEL_RAIN_DENSITY && local < PIXEL_FADE_MS {
            let value = scale(c.base_v, (255 - local * 255 / PIXEL_FADE_MS) as u8);
            (hash_u8(gen ^ i as u32), value)
        } else {
            (c.h, 0)
        }
    });
}

/// Pixel flow: a flowing hue curtain — each LED's hue is its row plus a per-column random
/// lane offset, the whole field drifting over time so colour appears to flow down columns.
fn render_pixel_flow(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let lane = hash_u8(grid_col(i) as u32);
        let hue = c
            .h
            .wrapping_add(lane)
            .wrapping_add(axis_hue(grid_row(i), GRID_H))
            .wrapping_add(phase);
        (hue, c.base_v)
    });
}

/// Twinkle brightness `0..=255` of chain index `i` at phase `t`: a triangle pulse with a
/// per-LED phase offset, so the LEDs shimmer out of step rather than pulsing in unison —
/// the shared body of the starlight effects.
fn starlight_pulse(i: usize, t: u64) -> u8 {
    let off = hash_u8(i as u32) as u64 * STARLIGHT_PERIOD_MS / 256;
    tri8(((t + off) % STARLIGHT_PERIOD_MS * 255 / STARLIGHT_PERIOD_MS) as u8)
}

/// Starlight: every LED softly twinkles in the base hue, each with a random phase.
fn render_starlight(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| (c.h, scale(c.base_v, starlight_pulse(i, c.t))));
}

/// Starlight (dual hue): as [`render_starlight`] but each LED twinkles in the base hue or
/// its complement, chosen by a per-LED hash.
fn render_starlight_dual_hue(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| {
        let hue = if hash_u8((i as u32) ^ 0xa5) & 1 == 0 {
            c.h
        } else {
            c.h.wrapping_add(128)
        };
        (hue, scale(c.base_v, starlight_pulse(i, c.t)))
    });
}

/// Solid multisplash: the [`reactive_splash`] rings from every recent press composited
/// over a dim base-colour floor rather than a dark background.
fn render_solid_multisplash(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    let floor = scale(c.base_v, REACTIVE_FLOOR);
    paint(c, leds, |i| {
        let (hue, v) = reactive_splash(i, c.h, c.base_v, &frame);
        (hue, v.max(floor))
    });
}

/// Solid reactive multiwide: every recent press blooms over its grid neighbours (the
/// [`reactive_wide`] falloff) over a dim base-colour floor.
fn render_solid_reactive_multiwide(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    let floor = scale(c.base_v, REACTIVE_FLOOR);
    paint(c, leds, |i| {
        let (hue, v) = reactive_wide(i, c.h, c.base_v, &frame);
        (hue, v.max(floor))
    });
}

/// Multinexus: every recent press lights a soft cross (the [`reactive_nexus`] row/column
/// falloff) over a dim base-colour floor.
fn render_multinexus(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    let floor = scale(c.base_v, REACTIVE_FLOOR);
    paint(c, leds, |i| (c.h, scale(c.base_v, reactive_nexus(i, &frame)).max(floor)));
}

// === Second QMK RGB Matrix batch (ids 38..=49) =============================
//
// The remaining families that close the headline 38→50 gap. Same rules as the block above
// (one `render_*` fn + one [`RGB_EFFECTS`] entry each, integer-only, O(LED_COUNT)/frame);
// the reactive ones reuse [`ReactiveFrame`] and the spatial helpers, and the two framebuffer
// effects keep a small per-LED `u8` state buffer.

/// Highest real key-grid row in [`LED_ROW`] (the bottom key row); with row `0` it bounds the
/// modifier frame the two-hue [`render_alphas_mods`] split uses.
const KEY_GRID_MAX_ROW: u8 = 5;
/// Highest real key-grid column in [`LED_COL`] (the right edge); the other perimeter bound.
const KEY_GRID_MAX_COL: u8 = 14;

/// Whether key LED `i` is a "modifier" for [`render_alphas_mods`]. The firmware carries no
/// per-key role map, so this is a spatial proxy: the perimeter frame (top and bottom rows
/// plus the outer columns — the function/modifier band) versus the interior letter block.
/// Only real key LEDs qualify; the tail LEDs (no key) fall through to the alpha hue and are
/// overwritten by the indicators anyway.
fn is_mod_led(i: usize) -> bool {
    is_key_led(i)
        && (LED_ROW[i] == 0
            || LED_ROW[i] == KEY_GRID_MAX_ROW
            || LED_COL[i] == 0
            || LED_COL[i] == KEY_GRID_MAX_COL)
}

/// Alphas/mods: the interior letter block at the base hue, the perimeter modifier frame
/// ([`is_mod_led`]) at its complement — a static two-hue split.
fn render_alphas_mods(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint(c, leds, |i| {
        let hue = if is_mod_led(i) { c.h.wrapping_add(128) } else { c.h };
        (hue, c.base_v)
    });
}

/// Rainbow pinwheels: two mirrored rainbow arms (the bearing doubled so the wheel repeats
/// twice around the centre) rotating over time.
fn render_rainbow_pinwheels(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let spin = (c.t / PINWHEEL_STEP_MS) as u8;
    paint(c, leds, |i| {
        (c.h.wrapping_add(grid_angle(i).wrapping_mul(2)).wrapping_add(spin), c.base_v)
    });
}

/// Pixel fractal: a symmetric triangle-wave of brightness travelling outward from the centre
/// column (each column's phase set by its distance from centre), with a per-column hue jitter
/// for a pixelated texture.
fn render_pixel_fractal(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let mirror = (grid_col(i) as i32 - GRID_CX).unsigned_abs() as u8;
        let value = tri8(mirror.wrapping_mul(28).wrapping_sub(phase));
        (c.h.wrapping_add(hash_u8(mirror as u32) & 0x1f), scale(c.base_v, value))
    });
}

/// Riverflow: a flowing triangle wave of hue along the panel diagonal (column + row),
/// drifting over time like a current.
fn render_riverflow(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let phase = (c.t / CYCLE_SPACE_STEP_MS) as u8;
    paint(c, leds, |i| {
        let diag = grid_col(i).wrapping_add(grid_row(i)).wrapping_mul(20);
        (c.h.wrapping_add(tri8(diag.wrapping_add(phase))), c.base_v)
    });
}

/// Typing heatmap (framebuffer): cool [`HEAT`] by the elapsed wall-clock time, then map each
/// key's accumulated heat to colour — cold keys near blue, hot (rapidly typed) keys toward
/// red, dark when cold. Heat is added per press by [`note_key_press`]; the cool is keyed on
/// [`HEAT_LAST_MS`] so it is frame-rate independent and idempotent within a frame. The ramp
/// is a fixed full-saturation blue→red (the heatmap reads heat, not the colour knob).
fn render_typing_heatmap(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let now = c.now_ms as u32;
    let elapsed = now.wrapping_sub(HEAT_LAST_MS.load(Ordering::Relaxed));
    let cool = (elapsed / HEAT_COOL_MS).min(255) as u8;
    if cool > 0 {
        HEAT_LAST_MS.store(now, Ordering::Relaxed);
        for cell in HEAT.iter() {
            let h = cell.load(Ordering::Relaxed);
            if h != 0 {
                cell.store(h.saturating_sub(cool), Ordering::Relaxed);
            }
        }
    }
    paint_full(leds, |i| {
        let heat = HEAT[i].load(Ordering::Relaxed) as u16;
        let hue = (HUE_BLUE as u16 * (255 - heat) / 255) as u8;
        (hue, 255, scale(c.base_v, heat as u8))
    });
}

/// [`MODE_DIGITAL_RAIN`] framebuffer: each grid cell's drip brightness, advanced one fall
/// step at a time by [`render_digital_rain`]. Render-private (only the RGB task touches it),
/// so the `Relaxed` atomics carry no cross-task contention — they match the file's per-LED
/// state idiom ([`LAST_PRESS`]) without a separate lock.
static RAIN: [AtomicU8; LED_COUNT] = [const { AtomicU8::new(0) }; LED_COUNT];
/// Last fall-step index [`render_digital_rain`] advanced [`RAIN`] to, so the buffer steps
/// exactly once per new step (and not again for a same-frame zone render).
static RAIN_STEP: AtomicU32 = AtomicU32::new(0);
/// Milliseconds per downward step of [`MODE_DIGITAL_RAIN`] (one row of fall).
const RAIN_FALL_MS: u64 = 70;
/// Per-step spawn threshold gating how often a column starts a new drip (low = sparse rain).
const RAIN_DROP_DENSITY: u8 = 0x28;
/// Brightness a drip loses per row as it falls, so each drop leaves a fading trail.
const RAIN_TRAIL_FADE: u8 = 56;

/// Advance the [`MODE_DIGITAL_RAIN`] framebuffer one fall step: shift every column down a row
/// (fading the trail by [`RAIN_TRAIL_FADE`]) and seed the top row of each column with a fresh
/// bright drip or darkness, from a per-(column, `step`) [`hash_u8`].
fn rain_advance(step: u32) {
    for col in 0..GRID_W {
        for r in (1..GRID_H).rev() {
            let above = RAIN[(r - 1) * GRID_W + col].load(Ordering::Relaxed);
            RAIN[r * GRID_W + col].store(above.saturating_sub(RAIN_TRAIL_FADE), Ordering::Relaxed);
        }
        let spawn = hash_u8((col as u32) ^ step.wrapping_mul(0x9e37_79b1));
        RAIN[col].store(if spawn < RAIN_DROP_DENSITY { 255 } else { 0 }, Ordering::Relaxed);
    }
}

/// Digital rain (framebuffer): bright drips fall down the columns leaving fading trails.
/// The fall step is keyed on the absolute frame timestamp `now_ms`, not the speed-scaled
/// phase `t`, so it is identical for the base effect and every zone in one displayed frame
/// (which share one `now_ms` but each carry their own speed-scaled `t`). The [`RAIN_STEP`]
/// gate then advances [`RAIN`] at most once per displayed frame regardless of a zone's speed
/// — the fall rate is fixed at [`RAIN_FALL_MS`], an absolute-time framebuffer like the
/// heatmap's cool. Then paints the buffer in the base hue.
fn render_digital_rain(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let step = (c.now_ms / RAIN_FALL_MS) as u32;
    if RAIN_STEP.swap(step, Ordering::Relaxed) != step {
        rain_advance(step);
    }
    paint(c, leds, |i| (c.h, scale(c.base_v, RAIN[i].load(Ordering::Relaxed))));
}

/// Solid reactive (simple): a pressed key flares to the base value and fades; no hue shift
/// and no floor, background dark — the plainest reactive effect.
fn render_solid_reactive_simple(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| (c.h, scale(c.base_v, frame.src[i])));
}

/// Reactive nexus: a pressed key lights a soft cross (the [`reactive_nexus`] row/column
/// falloff) on a dark background — the no-floor counterpart of [`render_multinexus`].
fn render_reactive_nexus(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    paint(c, leds, |i| (c.h, scale(c.base_v, reactive_nexus(i, &frame))));
}

/// Reactive cross (solid): the most-recent press ([`ReactiveFrame::latest`]) lights a hard
/// row/column cross ([`cross_single`]) over a dim base floor.
fn render_solid_reactive_cross(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    let floor = scale(c.base_v, REACTIVE_FLOOR);
    let latest = frame.latest();
    paint(c, leds, |i| {
        (c.h, scale(c.base_v, cross_single(i, latest, &frame)).max(floor))
    });
}

/// Reactive multicross: every recent press lights a hard cross (the [`reactive_cross`] plus)
/// over a dim base floor — the floored counterpart of [`MODE_REACTIVE_CROSS`]'s dark background.
fn render_solid_reactive_multicross(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    let floor = scale(c.base_v, REACTIVE_FLOOR);
    paint(c, leds, |i| {
        let (_, v) = reactive_cross(i, c.h, c.base_v, &frame);
        (c.h, v.max(floor))
    });
}

/// Solid splash: a single expanding [`splash_ring`] from the most-recent press
/// ([`ReactiveFrame::latest`]) over a dim base floor.
fn render_solid_splash(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    let frame = ReactiveFrame::capture(c.now_ms as u32, reactive_fade_ms(c.speed));
    let floor = scale(c.base_v, REACTIVE_FLOOR);
    let latest = frame.latest();
    paint(c, leds, |i| {
        let (hue, v) = match latest {
            Some(j) if is_key_led(i) => splash_ring(i, j, c.h, &frame),
            _ => (c.h, 0),
        };
        (hue, scale(c.base_v, v).max(floor))
    });
}

/// Starlight (dual saturation): every LED twinkles ([`starlight_pulse`]) in the base hue,
/// each at full or half saturation by a per-LED hash — the saturation counterpart of
/// [`render_starlight_dual_hue`].
fn render_starlight_dual_sat(c: &RgbCtx, leds: &mut [Rgb; LED_COUNT]) {
    paint_full(leds, |i| {
        let sat = if hash_u8((i as u32) ^ 0x3c) & 1 == 0 { c.s } else { c.s / 2 };
        (c.h, sat, scale(c.base_v, starlight_pulse(i, c.t)))
    });
}

/// The effect registry, indexed by mode id: `RGB_EFFECTS[mode]` renders that mode's
/// frame, so the live mode selects its renderer in O(1) with no dynamic dispatch.
/// The array's [`MODE_COUNT`] length makes coverage structural — every mode
/// `0..MODE_COUNT` has exactly one entry, enforced by the compiler — and the index
/// comments pin each slot to its `MODE_*` id.
pub static RGB_EFFECTS: [fn(&RgbCtx, &mut [Rgb; LED_COUNT]); MODE_COUNT as usize] = [
    render_solid,            // MODE_SOLID
    render_breathing,        // MODE_BREATHING
    render_rainbow,          // MODE_RAINBOW
    render_gradient_ud,      // MODE_GRADIENT_UD
    render_gradient_lr,      // MODE_GRADIENT_LR
    render_cycle_ud,         // MODE_CYCLE_UD
    render_cycle_lr,         // MODE_CYCLE_LR
    render_band,             // MODE_BAND
    render_pinwheel,         // MODE_PINWHEEL
    render_raindrops,        // MODE_RAINDROPS
    render_reactive,         // MODE_REACTIVE
    render_reactive_wide,    // MODE_REACTIVE_WIDE
    render_reactive_cross,   // MODE_REACTIVE_CROSS
    render_splash,           // MODE_SPLASH
    render_reactive_rainbow, // MODE_REACTIVE_RAINBOW
    render_band_val,                 // MODE_BAND_VAL
    render_band_sat,                 // MODE_BAND_SAT
    render_band_pinwheel_val,        // MODE_BAND_PINWHEEL_VAL
    render_band_pinwheel_sat,        // MODE_BAND_PINWHEEL_SAT
    render_band_spiral_val,          // MODE_BAND_SPIRAL_VAL
    render_band_spiral_sat,          // MODE_BAND_SPIRAL_SAT
    render_cycle_out_in,             // MODE_CYCLE_OUT_IN
    render_cycle_out_in_dual,        // MODE_CYCLE_OUT_IN_DUAL
    render_cycle_spiral,             // MODE_CYCLE_SPIRAL
    render_rainbow_moving_chevron,   // MODE_RAINBOW_MOVING_CHEVRON
    render_hue_breathing,            // MODE_HUE_BREATHING
    render_hue_pendulum,             // MODE_HUE_PENDULUM
    render_hue_wave,                 // MODE_HUE_WAVE
    render_dual_beacon,              // MODE_DUAL_BEACON
    render_rainbow_beacon,           // MODE_RAINBOW_BEACON
    render_jellybean_raindrops,      // MODE_JELLYBEAN_RAINDROPS
    render_pixel_rain,               // MODE_PIXEL_RAIN
    render_pixel_flow,               // MODE_PIXEL_FLOW
    render_starlight,                // MODE_STARLIGHT
    render_starlight_dual_hue,       // MODE_STARLIGHT_DUAL_HUE
    render_solid_multisplash,        // MODE_SOLID_MULTISPLASH
    render_solid_reactive_multiwide, // MODE_SOLID_REACTIVE_MULTIWIDE
    render_multinexus,               // MODE_MULTINEXUS
    render_alphas_mods,               // MODE_ALPHAS_MODS
    render_rainbow_pinwheels,         // MODE_RAINBOW_PINWHEELS
    render_pixel_fractal,             // MODE_PIXEL_FRACTAL
    render_riverflow,                 // MODE_RIVERFLOW
    render_typing_heatmap,            // MODE_TYPING_HEATMAP
    render_digital_rain,              // MODE_DIGITAL_RAIN
    render_solid_reactive_simple,     // MODE_SOLID_REACTIVE_SIMPLE
    render_reactive_nexus,            // MODE_REACTIVE_NEXUS
    render_solid_reactive_cross,      // MODE_SOLID_REACTIVE_CROSS
    render_solid_reactive_multicross, // MODE_SOLID_REACTIVE_MULTICROSS
    render_solid_splash,              // MODE_SOLID_SPLASH
    render_starlight_dual_sat,        // MODE_STARLIGHT_DUAL_SAT
];

// === Status indicators =====================================================
//
// An optional overlay ([`set_indicators`]) drawn over the rendered effect each
// frame, mapping live device status onto a small cluster of the no-key tail LEDs
// (the 7th grid row, which carries no matrix key — see "Per-key reactive
// effects"), so it never fights a key's reactive glow. The colours are steady,
// changing only when the underlying status does, so the overlay adds no extra SPI
// frames while idle (the render loop still transmits only on a frame change).

/// Base chain index of the indicator cluster: the start of the no-key tail row.
/// These four LEDs fall within the Right side zone; [`apply_indicators`] runs last
/// (after the zone compositing in [`rgb_task`]), so status always wins here over a
/// linked, independent or disabled zone — and over a host direct frame.
const INDICATOR_BASE: usize = LED_COUNT - GRID_W;
/// Per-host transport hue (USB / BT1-3 / 2.4G) — item 4's "per-host colour".
const IND_HOST: usize = INDICATOR_BASE;
/// Connection / link state (cable, connected, pairing, dropped).
const IND_LINK: usize = INDICATOR_BASE + 1;
/// Battery level, red→green (wireless transports only).
const IND_BATTERY: usize = INDICATOR_BASE + 2;
/// Active non-base keymap layer.
const IND_LAYER: usize = INDICATOR_BASE + 3;

/// Indicator hue: pure red (link down / empty battery).
const HUE_RED: u8 = 0;
/// Indicator hue: green (link up / full battery).
const HUE_GREEN: u8 = 85;
/// Indicator hue: blue (pairing).
const HUE_BLUE: u8 = 170;
/// Hue step between successive active layers, for [`layer_hue`].
const LAYER_HUE_STEP: u8 = 64;

/// Base hue mapped to each output transport (item 4): distinct points around the
/// wheel so a glance at [`IND_HOST`] says which host is active — USB cyan, the
/// three BT profiles stepping blue→violet, the 2.4G dongle green. Arbitrary but
/// stable.
fn transport_hue(devs: Devs) -> u8 {
    match devs {
        Devs::Usb => 128,
        Devs::Bt1 => 160,
        Devs::Bt2 => 180,
        Devs::Bt3 => 200,
        Devs::G2_4 => 96,
    }
}

/// Hue for the [`IND_LINK`] indicator: green on USB (the cable is the link) or a
/// connected radio, blue while pairing, red otherwise (dropped / rejected / idle).
fn link_hue() -> u8 {
    match wireless::transport() {
        Devs::Usb => HUE_GREEN,
        _ => match wireless::md::state() {
            MdState::Connected => HUE_GREEN,
            MdState::Pairing => HUE_BLUE,
            _ => HUE_RED,
        },
    }
}

/// Hue for the [`IND_BATTERY`] indicator (red at empty → green at full), or `None`
/// on USB where the radio battery reading is not meaningful (leave the effect).
fn battery_hue() -> Option<u8> {
    if wireless::transport() == Devs::Usb {
        return None;
    }
    let pct = wireless::battery().min(100) as u16;
    Some((pct * HUE_GREEN as u16 / 100) as u8)
}

/// Hue for the [`IND_LAYER`] indicator from the live active-layer mask
/// ([`telemetry::active_layers`]), or `None` when only the base layer is active
/// (nothing to flag). Higher active layers step the hue by [`LAYER_HUE_STEP`].
fn layer_hue() -> Option<u8> {
    let mask = telemetry::active_layers();
    let top = (u16::BITS - mask.leading_zeros()).checked_sub(1)?;
    if top == 0 {
        None
    } else {
        Some((top as u8).wrapping_mul(LAYER_HUE_STEP))
    }
}

/// Overlay the status indicators onto `leds` (called only when
/// [`indicators_enabled`]). Each indicator is the master `brightness` (capped to
/// [`MAX_BRIGHTNESS`]) at its status hue; the optional ones leave the underlying
/// effect in place when they have nothing to show.
fn apply_indicators(leds: &mut [Rgb; LED_COUNT], brightness: u8) {
    let value = brightness.min(MAX_BRIGHTNESS);
    leds[IND_HOST] = hsv_to_rgb(transport_hue(wireless::transport()), 255, value);
    leds[IND_LINK] = hsv_to_rgb(link_hue(), 255, value);
    if let Some(hue) = battery_hue() {
        leds[IND_BATTERY] = hsv_to_rgb(hue, 255, value);
    }
    if let Some(hue) = layer_hue() {
        leds[IND_LAYER] = hsv_to_rgb(hue, 255, value);
    }
}

// === Shared live state (written by the kcp RGB group, read by the task) ====
//
// Each field is an independent atomic with no cross-field invariant, so — like
// `crate::telemetry` — single-core `Relaxed` access is all the synchronisation
// needed between the kcp loop (writer) and the RGB task (reader); there is
// nothing to lock and no torn read to guard.

// Power-on defaults. Named so the live statics below and [`reset_defaults`]
// (the kcp CONFIG reset / restore-to-factory path) agree on one source of truth.
/// Default effect mode at power-on / reset.
const DEFAULT_MODE: u8 = MODE_RAINBOW;
/// Default effect hue.
const DEFAULT_HUE: u8 = 0;
/// Default effect saturation (fully saturated).
const DEFAULT_SAT: u8 = 255;
/// Default effect value.
const DEFAULT_VAL: u8 = 255;
/// Default master brightness.
const DEFAULT_BRIGHTNESS: u8 = 128;
/// Default animation speed. The mid value reproduces each effect's nominal
/// cadence (see [`effect_phase`]).
const DEFAULT_SPEED: u8 = 128;
/// Default enabled state (RGB on).
const DEFAULT_ENABLED: bool = true;
/// Default indicator-overlay state (status indicators on).
const DEFAULT_INDICATORS: bool = true;

/// Active effect mode (`MODE_*`).
static MODE: AtomicU8 = AtomicU8::new(DEFAULT_MODE);
/// Effect hue `0..=255`.
static HUE: AtomicU8 = AtomicU8::new(DEFAULT_HUE);
/// Effect saturation `0..=255`.
static SAT: AtomicU8 = AtomicU8::new(DEFAULT_SAT);
/// Effect value/brightness within the colour `0..=255`.
static VAL: AtomicU8 = AtomicU8::new(DEFAULT_VAL);
/// Master brightness knob `0..=255`. Combined with the effect value and then
/// clamped to [`MAX_BRIGHTNESS`] before emission, so the stored value may exceed
/// the panel cap (the GUI keeps a full 0..=255 range) but the light never does.
static BRIGHTNESS: AtomicU8 = AtomicU8::new(DEFAULT_BRIGHTNESS);
/// Animation speed `0..=255`. Scales the rate of every animating effect via
/// [`effect_phase`]; inert for the static effects (solid, the gradients).
static SPEED: AtomicU8 = AtomicU8::new(DEFAULT_SPEED);
/// Whether RGB output is enabled (false cuts the LED rail via PA8).
static ENABLED: AtomicBool = AtomicBool::new(DEFAULT_ENABLED);
/// Whether the status-indicator overlay is drawn over the effect
/// ([`apply_indicators`]). Persisted in the config blob ([`crate::config`]) from
/// schema v7, so the user's choice survives a reboot (before v7 it was live-only,
/// which surprised users when an indicator dot reappeared after every power-cycle).
static INDICATORS: AtomicBool = AtomicBool::new(DEFAULT_INDICATORS);

// === Zone table (composited over the base effect) ==========================
//
// The base effect (`MODE`/`HUE`/… above) renders the whole chain; the zone table
// then composites a small, fixed set of disjoint LED ranges over it ([`rgb_task`]).
// Each zone is **linked** (keeps the base effect's pixels in its range — the
// default, zero-cost, byte-for-byte today's look), **independent** (renders its own
// effect from the zone params) or **disabled** (its range is blanked). The status
// indicators stay a separate top overlay that always wins ([`apply_indicators`]).
//
// Unlike the per-field globals above, a zone carries a multi-field invariant (one
// `set_zone` updates seven bytes together) and the frame loop wants a coherent
// snapshot, so the table lives behind one brief blocking-mutex/`RefCell` lock — the
// same discipline as [`crate::config`] — rather than a fan of atomics.

/// Number of v1 zones the GUI lists (Keys, Right, Left); [`ZONE_CAP`] is the table
/// capacity, the spare slots reserved for host-defined (resizable) zones.
pub const ZONE_COUNT: usize = 3;
/// Zone-table capacity — the fixed slot count, `0..ZONE_CAP` addressable over kcp.
pub const ZONE_CAP: usize = 4;

/// Zone flag bit 0 — the zone is lit (clear blanks its LED range).
pub const ZONE_FLAG_ENABLED: u8 = 1 << 0;
/// Zone flag bit 1 — the zone mirrors the base effect's pixels in its range (clear
/// renders the zone's own effect from its params).
pub const ZONE_FLAG_LINKED: u8 = 1 << 1;
/// Default zone flags: enabled and linked, so an untouched board is the base effect
/// across the whole chain (today's behaviour, zero regression).
const ZONE_FLAGS_DEFAULT: u8 = ZONE_FLAG_ENABLED | ZONE_FLAG_LINKED;

/// [`Zone::sync_to`] sentinel for "not synced" — the zone shows its own effect rather
/// than mirroring another's. The default, so an untouched board is unchanged. This is
/// the in-RAM / kcp-request value; the GET_ZONE reply byte and the persisted config
/// slot carry the biased [`sync_to_wire`] encoding instead.
pub const ZONE_SYNC_NONE: u8 = 0xFF;

/// Encode a logical [`Zone::sync_to`] ([`ZONE_SYNC_NONE`] = not synced, else the target
/// zone id) into the GET_ZONE reply / persisted-config sync byte: `0` = not synced,
/// else `target_id + 1`. The `+1` bias is what makes a **zero** byte read back as "not
/// synced": a zeroed config slot decodes to none rather than "synced to zone 0". The
/// kcp `SET_ZONE_SYNC` *request* keeps the plain [`ZONE_SYNC_NONE`] (`0xFF`) sentinel,
/// since a command payload carries no zero-fill ambiguity. Inverse of [`sync_from_wire`].
pub const fn sync_to_wire(sync_to: u8) -> u8 {
    match sync_to {
        ZONE_SYNC_NONE => 0,
        target => target + 1,
    }
}

/// Decode a GET_ZONE / persisted sync byte ([`sync_to_wire`]) back to the logical
/// [`Zone::sync_to`]: `0` → [`ZONE_SYNC_NONE`], else `byte - 1`.
pub const fn sync_from_wire(byte: u8) -> u8 {
    match byte {
        0 => ZONE_SYNC_NONE,
        n => n - 1,
    }
}

/// One lighting zone: a half-open chain range (`start..start+count`) plus the effect
/// parameters it shows when independent. The table is [`ZONE_CAP`] of these.
#[derive(Clone, Copy)]
struct Zone {
    /// First chain index of the zone's LED range.
    start: u16,
    /// Number of LEDs in the range (`0` = inert).
    count: u16,
    /// Flags: [`ZONE_FLAG_ENABLED`] | [`ZONE_FLAG_LINKED`] (other bits reserved).
    flags: u8,
    /// Independent effect mode (`MODE_*`), shown when the zone is not linked.
    mode: u8,
    /// Independent hue `0..=255`.
    h: u8,
    /// Independent saturation `0..=255`.
    s: u8,
    /// Independent value `0..=255`.
    v: u8,
    /// Independent master brightness (folded and clamped like the base effect's).
    bright: u8,
    /// Independent animation speed `0..=255`.
    speed: u8,
    /// Sync source: another zone id whose effect this zone mirrors in its own range,
    /// or [`ZONE_SYNC_NONE`] when the zone shows its own effect. See [`set_zone_sync`].
    sync_to: u8,
}

impl Zone {
    /// A default zone over `start..start+count`: enabled and linked to the base
    /// effect, with the power-on colour params (dormant while linked) and no sync.
    const fn new(start: u16, count: u16) -> Self {
        Self {
            start,
            count,
            flags: ZONE_FLAGS_DEFAULT,
            mode: DEFAULT_MODE,
            h: DEFAULT_HUE,
            s: DEFAULT_SAT,
            v: DEFAULT_VAL,
            bright: DEFAULT_BRIGHTNESS,
            speed: DEFAULT_SPEED,
            sync_to: ZONE_SYNC_NONE,
        }
    }

    /// The zone's LED slice `start..start+count`, clamped to the chain so an
    /// already-validated range can never index out of bounds.
    fn range(&self) -> core::ops::Range<usize> {
        let start = (self.start as usize).min(LED_COUNT);
        let end = start.saturating_add(self.count as usize).min(LED_COUNT);
        start..end
    }
}

/// The power-on zone table: the three v1 zones tiling the chain (Keys, Right, Left),
/// plus one reserved empty slot a host may range into later.
const DEFAULT_ZONES: [Zone; ZONE_CAP] = [
    Zone::new(KEYS_START, KEYS_COUNT),   // id 0 — Keys
    Zone::new(RIGHT_START, RIGHT_COUNT), // id 1 — Right side strip
    Zone::new(LEFT_START, LEFT_COUNT),   // id 2 — Left side strip
    Zone::new(0, 0),                     // id 3 — reserved (inert until ranged)
];

/// The live zone table. Behind a blocking-mutex/`RefCell` (see the section note):
/// the kcp RGB group writes whole zones, [`rgb_task`] snapshots it each frame.
static ZONES: Mutex<CriticalSectionRawMutex, RefCell<[Zone; ZONE_CAP]>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new(DEFAULT_ZONES));

// === Host direct-streaming scaffold (`0x6C RGB_DIRECT`) ====================
//
// The OpenRGB/SignalRGB "Direct mode" escape hatch: a host streams a per-LED frame
// over kcp and the firmware shows it verbatim, bypassing the base+zone effects. A
// watchdog auto-reverts to the onboard zone effects when the stream stops (ASUS
// Aura `ReleaseControl`), so a closed app never leaves the board stuck on a stale
// host frame. The host streaming *engine* is deferred (vN); this is the firmware
// half only.

/// Host direct-stream framebuffer: the `0x6C` path writes per-LED colours here, and
/// [`rgb_task`] copies it straight to the chain while [`DIRECT_ACTIVE`].
static DIRECT_LEDS: Mutex<CriticalSectionRawMutex, RefCell<[Rgb; LED_COUNT]>> =
    Mutex::const_new(CriticalSectionRawMutex::new(), RefCell::new([Rgb::BLACK; LED_COUNT]));
/// Whether a host currently owns the frame via direct-streaming.
static DIRECT_ACTIVE: AtomicBool = AtomicBool::new(false);
/// Boot-uptime millisecond (`u32`, wrapping) of the last direct write; the watchdog
/// releases control once the stream is idle for [`DIRECT_TIMEOUT_MS`].
static DIRECT_LAST_MS: AtomicU32 = AtomicU32::new(0);
/// Direct-stream idle timeout: control reverts to the zone effects after this long
/// without a `0x6C` write (the implicit `ReleaseControl`).
const DIRECT_TIMEOUT_MS: u32 = 1000;

/// Set the effect mode. Returns `false` (and changes nothing) for an
/// out-of-range id, so the kcp handler can report `BadArg`.
#[must_use]
pub fn set_mode(id: u8) -> bool {
    if id < MODE_COUNT {
        MODE.store(id, Ordering::Relaxed);
        true
    } else {
        false
    }
}

/// Set the effect colour (hue, saturation, value); every component is valid.
pub fn set_hsv(h: u8, s: u8, v: u8) {
    HUE.store(h, Ordering::Relaxed);
    SAT.store(s, Ordering::Relaxed);
    VAL.store(v, Ordering::Relaxed);
}

/// Set the master brightness `0..=255`.
pub fn set_brightness(value: u8) {
    BRIGHTNESS.store(value, Ordering::Relaxed);
}

/// Set the animation speed `0..=255`.
pub fn set_speed(value: u8) {
    SPEED.store(value, Ordering::Relaxed);
}

/// Enable or disable RGB output (disabling cuts the LED rail).
pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::Relaxed);
}

/// Enable or disable the status-indicator overlay ([`apply_indicators`]).
pub fn set_indicators(on: bool) {
    INDICATORS.store(on, Ordering::Relaxed);
}

/// Current effect mode (`MODE_*`).
pub fn mode() -> u8 {
    MODE.load(Ordering::Relaxed)
}

/// Current effect colour `(h, s, v)`.
pub fn hsv() -> (u8, u8, u8) {
    (
        HUE.load(Ordering::Relaxed),
        SAT.load(Ordering::Relaxed),
        VAL.load(Ordering::Relaxed),
    )
}

/// Current master brightness `0..=255`.
pub fn brightness() -> u8 {
    BRIGHTNESS.load(Ordering::Relaxed)
}

/// Current animation speed `0..=255`.
pub fn speed() -> u8 {
    SPEED.load(Ordering::Relaxed)
}

/// Whether RGB output is currently enabled.
pub fn enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Whether the status-indicator overlay is currently drawn.
pub fn indicators_enabled() -> bool {
    INDICATORS.load(Ordering::Relaxed)
}

/// A snapshot of one zone's configurable state — the kcp `GET_ZONE` reply and the
/// persisted [`crate::config`] block.
pub struct ZoneState {
    /// Flags ([`ZONE_FLAG_ENABLED`] | [`ZONE_FLAG_LINKED`]).
    pub flags: u8,
    /// Independent effect mode (`MODE_*`).
    pub mode: u8,
    /// Independent hue `0..=255`.
    pub h: u8,
    /// Independent saturation `0..=255`.
    pub s: u8,
    /// Independent value `0..=255`.
    pub v: u8,
    /// Independent master brightness `0..=255`.
    pub bright: u8,
    /// Independent animation speed `0..=255`.
    pub speed: u8,
    /// First chain index of the zone's LED range.
    pub start: u16,
    /// Number of LEDs in the range.
    pub count: u16,
    /// Sync source zone id, or [`ZONE_SYNC_NONE`] when the zone shows its own effect.
    pub sync_to: u8,
}

/// Snapshot zone `id`'s state for the kcp `GET_ZONE` reply / the config blob, or
/// `None` when `id >= `[`ZONE_CAP`].
pub fn zone(id: usize) -> Option<ZoneState> {
    if id >= ZONE_CAP {
        return None;
    }
    Some(ZONES.lock(|cell| {
        let z = cell.borrow()[id];
        ZoneState {
            flags: z.flags,
            mode: z.mode,
            h: z.h,
            s: z.s,
            v: z.v,
            bright: z.bright,
            speed: z.speed,
            start: z.start,
            count: z.count,
            sync_to: z.sync_to,
        }
    }))
}

/// Set zone `id`'s effect params (the kcp `SET_ZONE` op): flags, effect mode and the
/// independent colour `(h, s, v)`, brightness and speed. The LED range is set
/// separately ([`set_zone_range`]). Returns `false` (changing nothing) for an
/// out-of-range `id` or `mode`, so the handler can report `BadArg`. Applied live.
///
/// Enabling a zone — an **off→on** transition of [`ZONE_FLAG_ENABLED`] — is also
/// rejected (returns `false`) when the zone's separately-set range overlaps another
/// enabled, non-empty zone: a zone disabled while another was resized over its old
/// range would otherwise re-light into an overlap the disjoint-range compositor cannot
/// represent. Only the *transition* is guarded (not a steady re-write), so a config
/// replay — where every zone starts enabled at boot, so nothing transitions off→on —
/// restores verbatim, exactly as the bounds-only [`set_zone_range`] does.
#[must_use]
pub fn set_zone(id: usize, flags: u8, mode: u8, hsv: (u8, u8, u8), bright: u8, speed: u8) -> bool {
    if id >= ZONE_CAP || mode >= MODE_COUNT {
        return false;
    }
    if flags & ZONE_FLAG_ENABLED != 0 {
        // Snapshot the current enabled bit and range under their own lock first: the
        // overlap probe locks the table itself, so it must not nest inside the write
        // borrow below. An empty range overlaps nothing ([`zone_range_overlaps`]).
        let (was_enabled, start, count) = ZONES.lock(|cell| {
            let z = &cell.borrow()[id];
            (z.flags & ZONE_FLAG_ENABLED != 0, z.start, z.count)
        });
        if !was_enabled && zone_range_overlaps(id, start, count) {
            return false;
        }
    }
    ZONES.lock(|cell| {
        let mut table = cell.borrow_mut();
        let z = &mut table[id];
        z.flags = flags;
        z.mode = mode;
        z.h = hsv.0;
        z.s = hsv.1;
        z.v = hsv.2;
        z.bright = bright;
        z.speed = speed;
    });
    true
}

/// Set zone `id`'s chain range (the kcp `SET_ZONE_RANGE` op). Returns `false`
/// (changing nothing) for an out-of-range `id` or a range past the chain
/// (`start + count > `[`LED_COUNT`]), so the handler can report `BadArg`. Applied
/// live.
///
/// This is the bounds-only setter: it does **not** itself reject overlap with another
/// zone, so the [`crate::config`] restore can replay an already-validated saved table
/// verbatim (an incremental replay would otherwise trip transient overlaps). The kcp
/// `SET_ZONE_RANGE` handler first consults [`zone_range_overlaps`] to enforce the
/// disjoint-lit-ranges policy on a host-initiated resize.
#[must_use]
pub fn set_zone_range(id: usize, start: u16, count: u16) -> bool {
    if id >= ZONE_CAP || start as usize + count as usize > LED_COUNT {
        return false;
    }
    ZONES.lock(|cell| {
        let mut table = cell.borrow_mut();
        table[id].start = start;
        table[id].count = count;
    });
    true
}

/// Whether placing zone `id` at `start..start+count` would overlap any **other**
/// enabled, non-empty zone. The compositing requires the lit zones to own disjoint
/// LED ranges (overlapping lit zones would fight for the same pixels in iteration
/// order), so the kcp `SET_ZONE_RANGE` handler rejects an overlapping resize with
/// `BadArg`. A disabled zone reserves no lit space (it only blanks its own range), so
/// it is not consulted; an empty proposed range (`count == 0`) overlaps nothing.
pub fn zone_range_overlaps(id: usize, start: u16, count: u16) -> bool {
    if count == 0 {
        return false;
    }
    let (new_start, new_end) = (start as usize, start as usize + count as usize);
    ZONES.lock(|cell| {
        cell.borrow().iter().enumerate().any(|(other, z)| {
            other != id
                && z.flags & ZONE_FLAG_ENABLED != 0
                && z.count != 0
                && new_start < z.start as usize + z.count as usize
                && (z.start as usize) < new_end
        })
    })
}

/// Set zone `id`'s sync source (the kcp `SET_ZONE_SYNC` op): a synced zone mirrors the
/// `target` zone's effect settings (enabled/linked, mode, colour, brightness, speed)
/// live in its **own** LED range. `target` is [`ZONE_SYNC_NONE`] to clear the link, or
/// a zone id. Returns `false` (changing nothing), so the handler can report `BadArg`,
/// for an out-of-range `id`/`target`, a self-sync, or a link that would close a cycle
/// (`a -> b -> a`) — the render only ever resolves a single hop, but a stored cycle is
/// rejected up front so the table stays a forest. Applied live.
#[must_use]
pub fn set_zone_sync(id: usize, target: u8) -> bool {
    if id >= ZONE_CAP {
        return false;
    }
    if target != ZONE_SYNC_NONE && (target as usize >= ZONE_CAP || target as usize == id) {
        return false;
    }
    ZONES.lock(|cell| {
        let mut table = cell.borrow_mut();
        // Reject a link that would close a cycle: walk the existing chain from the
        // target and bail if it reaches `id`. Bounded by ZONE_CAP hops, so a corrupt
        // pre-existing cycle not involving `id` terminates the walk rather than
        // spinning.
        if target != ZONE_SYNC_NONE {
            let mut cur = target as usize;
            for _ in 0..ZONE_CAP {
                if cur == id {
                    return false;
                }
                match table[cur].sync_to {
                    next if (next as usize) < ZONE_CAP => cur = next as usize,
                    _ => break,
                }
            }
        }
        table[id].sync_to = target;
        true
    })
}

/// Write a host direct-stream chunk into the [`DIRECT_LEDS`] framebuffer at chain
/// index `offset` (the kcp `RGB_DIRECT` op); `pixels` is a run of `r, g, b` triples.
/// Activates direct-streaming and resets the watchdog. Returns `false` (changing
/// nothing) when the chunk overruns the chain (`offset + pixels/3 > `[`LED_COUNT`]),
/// so the handler can report `BadArg`.
#[must_use]
pub fn direct_write(offset: usize, pixels: &[u8]) -> bool {
    let len = pixels.len() / 3;
    if offset.saturating_add(len) > LED_COUNT {
        return false;
    }
    DIRECT_LEDS.lock(|cell| {
        let mut buf = cell.borrow_mut();
        for (slot, px) in buf[offset..offset + len].iter_mut().zip(pixels.chunks_exact(3)) {
            *slot = Rgb {
                r: px[0],
                g: px[1],
                b: px[2],
            };
        }
    });
    DIRECT_LAST_MS.store(Instant::now().as_millis() as u32, Ordering::Relaxed);
    DIRECT_ACTIVE.store(true, Ordering::Relaxed);
    true
}

/// Snapshot the whole zone table for one render frame ([`rgb_task`]); the brief lock
/// is never held across the blocking SPI send.
fn snapshot_zones() -> [Zone; ZONE_CAP] {
    ZONES.lock(|cell| *cell.borrow())
}

/// Restore the power-on defaults (mode, colour, brightness, speed, enabled,
/// indicators and the zone table). Used by the kcp CONFIG reset-to-defaults path
/// ([`crate::config::reset_to_defaults`]); the render task picks the change up on
/// its next frame.
pub fn reset_defaults() {
    MODE.store(DEFAULT_MODE, Ordering::Relaxed);
    HUE.store(DEFAULT_HUE, Ordering::Relaxed);
    SAT.store(DEFAULT_SAT, Ordering::Relaxed);
    VAL.store(DEFAULT_VAL, Ordering::Relaxed);
    BRIGHTNESS.store(DEFAULT_BRIGHTNESS, Ordering::Relaxed);
    SPEED.store(DEFAULT_SPEED, Ordering::Relaxed);
    ENABLED.store(DEFAULT_ENABLED, Ordering::Relaxed);
    INDICATORS.store(DEFAULT_INDICATORS, Ordering::Relaxed);
    // Restore the power-on zone table (the three v1 zones, linked to the base).
    ZONES.lock(|cell| *cell.borrow_mut() = DEFAULT_ZONES);
    // Release any host direct-stream so a reset reverts to the zone effects.
    DIRECT_ACTIVE.store(false, Ordering::Relaxed);
}

// === WS2812-over-SPI encoding ==============================================

/// Encode one WS2812 colour byte's `pos`-th SPI byte (`pos` in `0..4`, MSB pair
/// first). Direct port of the QMK driver's `get_protocol_eq`
/// (ws2812_spi.c:123-134): each WS2812 bit becomes a 4-bit SPI nibble — `1` →
/// `0b1110`, `0` → `0b1000` — with the higher-order bit in the high nibble.
#[inline]
fn protocol_eq(data: u8, pos: usize) -> u8 {
    let shift = 2 * (3 - pos);
    let lo_bit = (data >> shift) & 1; // WS2812 bit 2*(3-pos)
    let hi_bit = (data >> (shift + 1)) & 1; // WS2812 bit 2*(3-pos)+1
    let mut eq: u8 = if lo_bit != 0 { 0b1110 } else { 0b1000 };
    eq += if hi_bit != 0 { 0b1110_0000 } else { 0b1000_0000 };
    eq
}

/// Encode the LED buffer into the SPI transmit buffer: 4-byte zero preamble,
/// then each LED's GRB bytes expanded four-SPI-bytes-each (ws2812_spi.c:139-145),
/// then the trailing zero reset latch. `txbuf` is exactly [`TXBUF_LEN`].
fn encode(leds: &[Rgb; LED_COUNT], txbuf: &mut [u8; TXBUF_LEN]) {
    txbuf[..PREAMBLE_SIZE].fill(0);

    let data = &mut txbuf[PREAMBLE_SIZE..PREAMBLE_SIZE + DATA_SIZE];
    for (i, led) in leds.iter().enumerate() {
        let base = SPI_BYTES_PER_LED * i;
        // GRB byte order.
        for (j, slot) in data[base..base + 4].iter_mut().enumerate() {
            *slot = protocol_eq(led.g, j);
        }
        for (j, slot) in data[base + 4..base + 8].iter_mut().enumerate() {
            *slot = protocol_eq(led.r, j);
        }
        for (j, slot) in data[base + 8..base + 12].iter_mut().enumerate() {
            *slot = protocol_eq(led.b, j);
        }
    }

    txbuf[PREAMBLE_SIZE + DATA_SIZE..].fill(0);
}

/// Shift the whole buffer out of SPIM2 by polling, draining the receive FIFO in
/// lock-step, then wait for the core to go idle so the trailing reset bytes have
/// left the pin.
///
/// `CR0` keeps the core in transmit-and-receive mode (matching the HAL), so each
/// transmitted byte produces a received one. The loop pushes a byte only while
/// the transmit FIFO has room *and* transmit is no more than [`FIFO_DEPTH`]
/// bytes ahead of receive, and discards every received byte — so the RX FIFO can
/// never overflow and gate the transmit clock. This is the polled equivalent of
/// the HAL's `rxtx_gap` throttle and RX drain (`spi_lld_serve_event_interrupt`,
/// hal_spi_lld.c:77-121). The core is left enabled and idle (data line low, last
/// reset byte = 0) between frames.
///
/// Synchronous: this busy-waits for the whole ≈3.67 ms transfer (see the
/// module-level "Executor cost" note); callers gate it behind change-detection.
/// Every spin is bounded: the RX drain reads at most `n` bytes (so a stuck-high
/// `RFNE` cannot wedge it), while the per-byte progress loop and the trailing
/// core-idle wait are capped by [`SPI_SEND_TIMEOUT`] — on a stalled core (RX never
/// advancing, or `BUSY` never clearing) the frame is abandoned and
/// [`SendResult::TimedOut`] returned so [`rgb_task`] can re-bring-up SPIM2. That is
/// the deliberate failure mode, chosen over an unbounded spin that would freeze the
/// whole single-core executor forever.
fn spim2_send(txbuf: &[u8; TXBUF_LEN]) -> SendResult {
    // SAFETY: `Spim2::ptr()` is the fixed-address SPIM2 register block, valid
    // for the whole program. Access is via the PAC's volatile proxies; only
    // this task touches SPIM2, so there is no aliasing or reentrancy hazard.
    let spi = unsafe { &*Spim2::ptr() };

    let depth = FIFO_DEPTH.load(Ordering::Relaxed) as usize;
    let n = txbuf.len();
    let mut tx_i = 0; // bytes pushed into the transmit FIFO
    let mut rx_i = 0; // bytes drained from the receive FIFO
    // Deadline guarding both busy-waits below; a healthy frame finishes well
    // inside it, so it only trips when the core has genuinely wedged.
    let deadline = Instant::now() + SPI_SEND_TIMEOUT;

    while rx_i < n {
        // Push while there is data left, transmit-FIFO room, and transmit has
        // not run more than `depth` frames ahead of receive (so the equal-depth
        // RX FIFO cannot overflow).
        if tx_i < n && tx_i - rx_i < depth && spi.sr().read().bits() & SPI_SR_TFNF != 0 {
            spi.dr().write(|w| unsafe { w.bits(txbuf[tx_i] as u32) });
            tx_i += 1;
        }
        // Drain received bytes (value discarded), capped at `n`: the lock-step
        // push above means at most `n` bytes are ever received, so this bound is
        // free in the healthy case and stops a stuck-high `RFNE` from spinning
        // here forever — which would never reach the deadline check below.
        while rx_i < n && spi.sr().read().bits() & SPI_SR_RFNE != 0 {
            let _ = spi.dr().read().bits();
            rx_i += 1;
        }
        if Instant::now() >= deadline {
            return SendResult::TimedOut;
        }
    }

    // Every byte has been shifted and received; wait for the core to finish, under
    // the same deadline.
    while spi.sr().read().bits() & SPI_SR_BUSY != 0 {
        if Instant::now() >= deadline {
            return SendResult::TimedOut;
        }
    }
    SendResult::Ok
}

/// Outcome of a polled [`spim2_send`].
enum SendResult {
    /// The whole frame shifted out and the core went idle within the deadline.
    Ok,
    /// The [`SPI_SEND_TIMEOUT`] deadline passed mid-transfer — the core is assumed
    /// wedged, so the frame was abandoned. The caller re-runs [`init`].
    TimedOut,
}

// === Bring-up ==============================================================

/// Configure PB15 as the SPIM2 data output (alternate function, push-pull,
/// max drive) using the `CFGMSK` write-mask idiom from [`crate::gpio`]: expose
/// only PB15, then write whole configuration registers — every other pin's
/// field is protected by its `CFGMSK` bit. Drive strength is set with a
/// read-modify-write since `CFGMSK` does not gate the `CURRENT` register.
fn configure_data_pin(p: &Peripherals) {
    let n = WS2812_DATA_PIN as u32;

    // Expose only PB15 for the whole-register configuration writes below.
    p.gpiob.cfgmsk().write(|w| unsafe { w.bits(!(1u32 << WS2812_DATA_PIN)) });
    p.gpiob
        .moder()
        .write(|w| unsafe { w.bits(MODER_ALTERNATE << (2 * n)) });
    // Push-pull (OTYPER 0), no pull resistor (PUPDR 0).
    p.gpiob.otyper().write(|w| unsafe { w.bits(0) });
    p.gpiob.pupdr().write(|w| unsafe { w.bits(0) });
    p.gpiob
        .afrh()
        .write(|w| unsafe { w.bits(WS2812_DATA_AF << (4 * (n - 8))) });
    // Drive strength: RMW so other pins' CURRENT fields are preserved.
    p.gpiob.current().modify(|r, w| unsafe {
        w.bits((r.bits() & !(0b11 << (2 * n))) | (CURRENT_MAX << (2 * n)))
    });
}

/// Bring up SPIM2 and the LED pins. Call once from `main` after the clock is up
/// and before [`rgb_task`] runs.
///
/// Enables the SPIM2 clock, pulses its reset, configures PB15 (data) and PA8
/// (rail-enable, driven high), then programs SPIM2 — a polled port of the WB32
/// HAL `spi_lld_start` (hal_spi_lld.c:241-322): baud divisor, slave-select,
/// `CR0`, cleared FIFO thresholds and interrupts — and finally enables the core.
pub fn init(p: &Peripherals) {
    // SPIM2 peripheral clock on, then pulse its reset (set, clear), each with
    // the usual read-back write barrier (`rccEnableSPIM2`/`rccResetSPIM2`,
    // wb32_rcc.h:348/362).
    p.rcc
        .apb2enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB2ENR_SPIM2EN) });
    let _ = p.rcc.apb2enr().read().bits();
    p.rcc
        .apb2rstr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB2RSTR_SPIM2RST) });
    p.rcc
        .apb2rstr()
        .modify(|r, w| unsafe { w.bits(r.bits() & !RCC_APB2RSTR_SPIM2RST) });
    let _ = p.rcc.apb2rstr().read().bits();

    // Ensure the GPIOA (PA8) and GPIOB (PB15) clocks are on (read-modify-write
    // so the matrix/clock bring-up's enables stay set).
    p.rcc
        .apb1enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB1ENR_GPIOAEN | RCC_APB1ENR_GPIOBEN) });
    let _ = p.rcc.apb1enr().read().bits();

    configure_data_pin(p);

    // LED rail-enable PA8 + boost-converter enable PA9: push-pull outputs, both
    // driven high to power the WS2812 chain (the donor asserts both, ansi.c:201).
    gpio::set_output_push_pull(LED_POWER_EN);
    gpio::set_high(LED_POWER_EN);
    gpio::set_output_push_pull(LED_BOOST_EN);
    gpio::set_high(LED_BOOST_EN);

    // Program SPIM2 while the core is disabled (DW-SSI requires CR0/BAUDR/SER
    // to be written with the core off), then enable it. Mirrors spi_lld_start.
    // SAFETY: see `spim2_send`.
    let spi = unsafe { &*Spim2::ptr() };
    spi.spienr().write(|w| unsafe { w.bits(SPI_SPIENR_DIS) });
    spi.baudr().write(|w| unsafe { w.bits(SPI_BAUDR_DIV32) });
    spi.ser().write(|w| unsafe { w.bits(SPI_SER_SE0) });
    spi.cr0().write(|w| unsafe { w.bits(SPI_CR0_WS2812) });

    // Probe the FIFO depth (port of hal_spi_lld.c:304-313): TXFTLR holds
    // 0..depth-1, so the first value that fails to read back marks the depth.
    // `spim2_send` uses it to throttle transmit so the RX FIFO never overflows.
    let mut depth: u32 = 1;
    while depth < 256 {
        spi.txftlr().write(|w| unsafe { w.bits(depth) });
        if spi.txftlr().read().bits() != depth {
            break;
        }
        depth += 1;
    }
    FIFO_DEPTH.store(depth.max(1) as u16, Ordering::Relaxed);

    spi.txftlr().write(|w| unsafe { w.bits(0) });
    spi.rxftlr().write(|w| unsafe { w.bits(0) });
    spi.ier().write(|w| unsafe { w.bits(0) }); // polled: no interrupts
    let _ = spi.icr().read().bits(); // clear any pending interrupt flags
    spi.spienr().write(|w| unsafe { w.bits(SPI_SPIENR_EN) });
}

/// RGB render loop: own the LED and SPI buffers, and at [`FRAME_INTERVAL_US`]
/// (~30 Hz) read the live state, render the active effect (brightness folded in
/// and clamped to [`MAX_BRIGHTNESS`]) and push the frame to the WS2812 chain —
/// but only when the rendered frame *changed*, so a static effect transmits once
/// and then costs nothing (the WS2812s hold their latched state). When disabled
/// it cuts the LED rail (PA8 low) and skips the transfer until re-enabled.
///
/// Independent of USB — spawned as its own task from `main`. See the module
/// "Executor cost" note for why the send is gated and rate-capped.
#[embassy_executor::task]
pub async fn rgb_task() {
    let mut leds = [Rgb::BLACK; LED_COUNT];
    // Last frame actually transmitted, for change detection.
    let mut last_sent = [Rgb::BLACK; LED_COUNT];
    let mut txbuf = [0u8; TXBUF_LEN];
    // Scratch frame an independent zone renders into; only its range is copied onto
    // the live frame, so the other zones' pixels are untouched. Reused across zones.
    let mut zone_scratch = [Rgb::BLACK; LED_COUNT];
    // [`init`] left the rail powered (PA8 high); track it so the pin is only
    // toggled on an enabled/disabled edge.
    let mut rail_powered = true;
    // Force the next transmit regardless of change detection (first frame, and
    // after the rail was re-powered — the LEDs lost their latched state).
    let mut force_send = true;

    loop {
        Timer::after(Duration::from_micros(FRAME_INTERVAL_US)).await;

        if !enabled() {
            if rail_powered {
                gpio::set_low(LED_POWER_EN);
                rail_powered = false;
            }
            continue;
        }
        if !rail_powered {
            gpio::set_high(LED_POWER_EN);
            rail_powered = true;
            force_send = true;
        }

        let bright = brightness();
        let now_ms = Instant::now().as_millis();
        let now = now_ms as u32;

        // Host direct-streaming (the `0x6C` scaffold): while a host owns the frame and
        // its watchdog has not expired, show its buffer verbatim and skip the base+zone
        // effects (the indicators still overlay last, below). When the stream goes idle
        // past the watchdog, release control back to the zone effects (ASUS Aura
        // `ReleaseControl`) and force the revert frame out.
        let direct_active = DIRECT_ACTIVE.load(Ordering::Relaxed);
        let direct = direct_active
            && now.wrapping_sub(DIRECT_LAST_MS.load(Ordering::Relaxed)) < DIRECT_TIMEOUT_MS;
        if direct_active && !direct {
            DIRECT_ACTIVE.store(false, Ordering::Relaxed);
            force_send = true;
        }

        if direct {
            DIRECT_LEDS.lock(|cell| leds.copy_from_slice(&cell.borrow()[..]));
        } else {
            // Base effect from the registry: the one effect that owns the live mode
            // renders the whole chain. `base_v` folds the master brightness in once;
            // `t`/`now_ms` carry the animation phase and the reactive frame timestamp.
            // A feature may claim (veto) the whole frame for a status overlay; none
            // does today. `mode()` is always a validated id in `0..MODE_COUNT`, so it
            // indexes its renderer in O(1); `.get` keeps a no-op fallback for an
            // (unreachable) out-of-range mode.
            let (h, s, v) = hsv();
            let sp = speed();
            let m = mode();
            let ctx = RgbCtx {
                h,
                s,
                base_v: scale(v, bright),
                speed: sp,
                now_ms,
                t: effect_phase(now_ms, sp),
            };
            if !features::run_on_rgb_frame(&ctx, &mut leds) {
                if let Some(render) = RGB_EFFECTS.get(m as usize) {
                    render(&ctx, &mut leds);
                }
            }
            // Zone compositing: each zone owns a disjoint LED range over the base
            // effect. Disabled blanks the range; linked (the default) keeps the base
            // effect's pixels there — zero cost, today's look; independent renders the
            // zone's own effect into the scratch frame and copies just its range. A
            // synced zone draws its *target* zone's effect in its own range (resolved
            // one hop only — `set_zone_sync` rejects cycles, so this never chains). The
            // status indicators still overlay last, so they win on the tail LEDs
            // regardless of the zone config.
            let table = snapshot_zones();
            for (id, z) in table.iter().enumerate() {
                let range = z.range();
                if range.is_empty() {
                    continue;
                }
                // The effect source: the sync target when one hop away, else the zone
                // itself. Only the *range* stays the zone's own.
                let src = match z.sync_to {
                    t if (t as usize) < ZONE_CAP && t as usize != id => &table[t as usize],
                    _ => z,
                };
                if src.flags & ZONE_FLAG_ENABLED == 0 {
                    leds[range].fill(Rgb::BLACK);
                } else if src.flags & ZONE_FLAG_LINKED == 0 {
                    let zctx = RgbCtx {
                        h: src.h,
                        s: src.s,
                        base_v: scale(src.v, src.bright),
                        speed: src.speed,
                        now_ms,
                        t: effect_phase(now_ms, src.speed),
                    };
                    if let Some(render) = RGB_EFFECTS.get(src.mode as usize) {
                        render(&zctx, &mut zone_scratch);
                    }
                    leds[range.clone()].copy_from_slice(&zone_scratch[range]);
                }
                // else linked: keep the base effect's pixels (no work).
            }
        }

        // Overlay live status onto the tail cluster, always last so it wins over the
        // base effect, any zone and a host direct frame. The indicator colours are
        // steady, so this changes the frame only when the status itself does and adds
        // no SPI frames while idle (the change-detection below still gates).
        if indicators_enabled() {
            apply_indicators(&mut leds, bright);
        }

        // Pay the blocking SPI transfer only when the frame changed (or a resend
        // is forced); an unchanging effect sends once and then idles.
        if force_send || leds != last_sent {
            encode(&leds, &mut txbuf);
            match spim2_send(&txbuf) {
                SendResult::Ok => {
                    last_sent = leds;
                    force_send = false;
                }
                SendResult::TimedOut => {
                    // The SPI core wedged past SPI_SEND_TIMEOUT; the send was
                    // abandoned before it could freeze the executor. Re-bring-up
                    // SPIM2 and force a full resend next frame (the abort left the
                    // WS2812 stream partially clocked).
                    defmt::warn!("rgb: SPIM2 send timed out, re-initialising");
                    // SAFETY: `main` has dropped its `Peripherals` by the time the
                    // spawned tasks run, so no live `&Peripherals` aliases this; the
                    // RGB task is the sole owner of the SPIM2 / LED-pin registers it
                    // re-programs.
                    let p = unsafe { Peripherals::steal() };
                    init(&p);
                    force_send = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Re-enabling a zone whose range another was resized over while it was disabled
    /// must be rejected, so the lit zones stay disjoint for the compositor (the
    /// `set_zone` enable-transition guard).
    #[test]
    fn reenabling_into_an_overlap_is_rejected() {
        reset_defaults();
        // Disable Keys (zone 0), leaving its 0..83 range reserved but dark.
        assert!(set_zone(0, 0, DEFAULT_MODE, (0, 0, 0), 0, 0));
        // Resize the spare zone 3 over Keys' now-dark range — allowed while Keys is off.
        assert!(set_zone_range(3, 0, 40));
        // Re-enabling Keys (0..83) would overlap the lit zone 3 (0..40): rejected.
        assert!(!set_zone(0, ZONE_FLAG_ENABLED, DEFAULT_MODE, (0, 0, 0), 0, 0));
        reset_defaults();
    }

    /// The GET_ZONE / config sync byte is biased so a zero — a zeroed config slot —
    /// decodes as "not synced", and a real target round-trips through the `+1` bias.
    #[test]
    fn sync_wire_encoding_biases_none_to_zero() {
        assert_eq!(sync_to_wire(ZONE_SYNC_NONE), 0);
        assert_eq!(sync_from_wire(0), ZONE_SYNC_NONE);
        // Synced to zone 0 stores 1 on the wire (never 0), and round-trips back.
        assert_eq!(sync_to_wire(0), 1);
        assert_eq!(sync_from_wire(1), 0);
    }
}
