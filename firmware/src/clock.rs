// SPDX-License-Identifier: GPL-2.0-or-later
//! 96 MHz PLL system-clock bring-up for the Westberry WB32FQ95.
//!
//! This is a faithful port of the WB32 vendor HAL clock initialisation
//! (ChibiOS-Contrib `os/hal/ports/WB32/WB32FQ95xx/hal_lld.c`, functions
//! `wb32_clock_init` and `SetSysClock`) specialised for the Akko 5075B board.
//!
//! Board configuration:
//!
//! | Parameter      | Value | Notes                                     |
//! | -------------- | ----- | ----------------------------------------- |
//! | HSE crystal    | 12 MHz| External crystal on PD0/PD1               |
//! | PLLDIV         | 2     | PLL input = HSE / PLLDIV = 12 / 2 = 6 MHz |
//! | PLLMUL         | 16    | PLL output = 6 MHz * 16 = 96 MHz          |
//! | MAINCLKSRC     | PLL   | System clock taken from the PLL           |
//! | AHB/APB1/APB2  | /1    | HCLK = PCLK1 = PCLK2 = 96 MHz             |
//!
//! At 96 MHz the flash/cache runs with 3 wait states (the `HCLK > 72 MHz`
//! branch of the vendor wait-state selection).
//!
//! The PAC models every peripheral register as a raw 32-bit word, so each
//! value below is composed from the vendor field constants and written with
//! `w.bits(..)`. Every constant carries the header/source line it derives
//! from so the bring-up can be cross-checked against the vendor sources.

use pac::Peripherals;

/// Resulting core/AHB clock (HCLK) frequency: HSE / PLLDIV * PLLMUL.
pub const HCLK_HZ: u32 = 96_000_000;

// === PWR: ANCTL write-unlock keys ===
// Source: hal_lld.c `wb32_clock_init`/`SetSysClock` (lines 51-52, 145-146,
// 266-267): `PWR->ANAKEY1 = 0x03; PWR->ANAKEY2 = 0x0C;` unlocks, `0x00` locks.

/// PWR.ANAKEY1 value that begins unlocking writes to the ANCTL block.
const ANAKEY1_UNLOCK: u32 = 0x03;
/// PWR.ANAKEY2 value that completes unlocking writes to the ANCTL block.
const ANAKEY2_UNLOCK: u32 = 0x0C;
/// PWR.ANAKEY1/ANAKEY2 value that re-locks the ANCTL block.
const ANAKEY_LOCK: u32 = 0x00;

// === ANCTL.PORCR: power-on-reset monitor ===
/// ANCTL.PORCR value that turns off the power-on reset monitor
/// (hal_lld.c:270, `ANCTL->PORCR = 0x7BE`).
const PORCR_POR_OFF: u32 = 0x7BE;

// === RCC APB prescalers ===
/// RCC.APB1PRE source enable; PPRE1 == 1 so no ratio bits are set
/// (`RCC_APB1PRE_SRCEN = 0x1U << 7`, wb32fq95xx.h:3455).
const RCC_APB1PRE_SRCEN: u32 = 0x1 << 7;
/// RCC.APB2PRE source enable; PPRE2 == 1 so no ratio bits are set
/// (`RCC_APB2PRE_SRCEN = 0x1U << 7`, wb32fq95xx.h:3525).
const RCC_APB2PRE_SRCEN: u32 = 0x1 << 7;

// === RCC peripheral clock-enable bits ===
/// RCC.APB1ENR: bus-matrix BMX1 clock enable
/// (`RCC_APB1ENR_BMX1EN = 0x1U << 15`, wb32fq95xx.h:4163).
const RCC_APB1ENR_BMX1EN: u32 = 0x1 << 15;
/// RCC.APB1ENR: GPIOD clock enable; PD0/PD1 carry the HSE crystal
/// (`RCC_APB1ENR_GPIODEN = 0x1U << 8`, wb32fq95xx.h:4156).
const RCC_APB1ENR_GPIODEN: u32 = 0x1 << 8;
/// RCC.APB2ENR: bus-matrix BMX2 clock enable
/// (`RCC_APB2ENR_BMX2EN = 0x1U << 11`, wb32fq95xx.h:4177).
const RCC_APB2ENR_BMX2EN: u32 = 0x1 << 11;

// === GPIOD: HSE crystal pad configuration (non-bypass path) ===
// Source: hal_lld.c:170-171 (the `#else` / crystal branch of `SetSysClock`).
/// GPIOD.CFGMSK write mask that exposes only PD0/PD1 for configuration.
const GPIOD_CFGMSK_HSE: u32 = 0xFFFC;
/// GPIOD.MODER value placing PD0 and PD1 in analog mode for the crystal.
const GPIOD_MODER_HSE: u32 = 0x0F;

// === ANCTL: HSE oscillator control ===
/// ANCTL.HSECR1: HSE pad output enable, the non-bypass crystal path
/// (`ANCTL_HSECR1_PADOEN = 0x1U << 1`, wb32fq95xx.h:3186).
const ANCTL_HSECR1_PADOEN: u32 = 0x1 << 1;
/// ANCTL.HSECR0: turn the HSE oscillator on
/// (`ANCTL_HSECR0_HSEON = 0x1U << 0`, wb32fq95xx.h:3182).
const ANCTL_HSECR0_HSEON: u32 = 0x1 << 0;
/// ANCTL.HSESR: external high-speed clock ready flag
/// (`ANCTL_HSESR_HSERDY = 0x1U << 0`, wb32fq95xx.h:3189).
const ANCTL_HSESR_HSERDY: u32 = 0x1 << 0;
/// Iteration cap for the HSE crystal start-up busy-wait. The vendor HAL uses
/// `HSE_STARTUP_TIMEOUT = 48000` (wb32fq95xx.h:66); this uses a more generous
/// cap, comfortably longer than real crystal start-up at the 8 MHz MHSI boot
/// clock, so that only a genuine hardware fault trips it.
const HSE_STARTUP_TIMEOUT: u32 = 1_000_000;

// === CACHE.CR: flash prefetch, cache and wait states ===
// Composed for the `WB32_MAINCLK > 72000000` branch (hal_lld.c:197):
// `CACHE_CR_CHEEN | CACHE_CR_PREFEN_ON | CACHE_CR_LATENCY_3WS`.
/// CACHE.CR: cache enable (`CACHE_CR_CHEEN = 0x3U << 24`, wb32fq95xx.h:3126).
const CACHE_CR_CHEEN: u32 = 0x3 << 24;
/// CACHE.CR: prefetch enable
/// (`CACHE_CR_PREFEN_ON = 0x1U << 4`, wb32fq95xx.h:3122).
const CACHE_CR_PREFEN_ON: u32 = 0x1 << 4;
/// CACHE.CR: 3 flash wait states for HCLK > 72 MHz
/// (`CACHE_CR_LATENCY_3WS = 0x3U`, wb32fq95xx.h:3106).
const CACHE_CR_LATENCY_3WS: u32 = 0x3;

// === RCC.AHBPRE: AHB prescaler ===
/// RCC.AHBPRE for HPRE == 1: AHB clock equals the main clock, no division
/// (hal_lld.c:202, `RCC->AHBPRE = 0x00`).
const RCC_AHBPRE_DIV1: u32 = 0x00;

// === RCC / ANCTL: PLL configuration ===
/// RCC.PLLSRC: select HSE as the PLL reference
/// (`RCC_PLLSRC_HSE = 0x1U`, wb32fq95xx.h:3298).
const RCC_PLLSRC_HSE: u32 = 0x1;
/// RCC.PLLPRE: enable the PLL pre-divider source
/// (`RCC_PLLPRE_SRCEN = 0x1U << 5`, wb32fq95xx.h:3294).
const RCC_PLLPRE_SRCEN: u32 = 0x1 << 5;
/// RCC.PLLPRE: enable the PLL pre-divider, required when PLLDIV != 1
/// (`RCC_PLLPRE_DIVEN = 0x1U << 0`, wb32fq95xx.h:3275).
const RCC_PLLPRE_DIVEN: u32 = 0x1 << 0;
/// Crystal PLL pre-divider value for this board (PLL input = HSE / PLLDIV).
/// The PLLPRE ratio field encodes `PLLDIV - 2` (hal_lld.c:226).
const PLLDIV: u32 = 2;
/// ANCTL.PLLCR: PLL multiplier == 16 encoding, `0x2U << 6`
/// (hal_lld.c:232-233, the `WB32_PLLMUL_VALUE == 16` branch).
const ANCTL_PLLCR_PLLMUL16: u32 = 0x2 << 6;
/// ANCTL.PLLENR: turn the PLL on
/// (`ANCTL_PLLENR_PLLON = 0x1U << 0`, wb32fq95xx.h:3199).
const ANCTL_PLLENR_PLLON: u32 = 0x1 << 0;
/// ANCTL.PLLSR PLL-locked ready value, which doubles as its mask: the lock bits
/// are `ANCTL_PLLSR_PLLRDY_Msk = 0x3U` (wb32fq95xx.h:3202), so the wait masks the
/// status register with this value and compares equal (hal_lld.c:243) rather than
/// requiring the whole register to read exactly `0x03` — a locked PLL reporting any
/// extra status bit must still pass.
const ANCTL_PLLSR_READY: u32 = 0x03;
/// Iteration cap for the PLL lock busy-wait, sized like [`HSE_STARTUP_TIMEOUT`].
/// The vendor HAL leaves this wait unbounded; bounding it lets a PLL fault fail
/// loudly over RTT instead of hanging forever.
const PLL_LOCK_TIMEOUT: u32 = 1_000_000;

// === RCC: main clock source selection ===
/// RCC.MAINCLKSRC: select the PLL as system clock
/// (`RCC_MAINCLKSRC_PLLCLK = 0x2U`, wb32fq95xx.h:3303).
const RCC_MAINCLKSRC_PLL: u32 = 0x2;
/// RCC.MAINCLKUEN: latch the new main-clock-source selection
/// (`RCC_MAINCLKUEN_ENA = 0x1U`, wb32fq95xx.h:3307).
const RCC_MAINCLKUEN_ENA: u32 = 0x1;

/// Bring the WB32FQ95 system clock up to 96 MHz from the 12 MHz HSE crystal.
///
/// Mirrors the vendor `wb32_clock_init`: unlock the ANCTL block, turn off the
/// power-on-reset monitor, run [`set_sys_clock`], then enable the BMX1/BMX2
/// bus-matrix clocks. (The LSI activation present in the vendor function is
/// omitted because LSI is disabled for this board.)
pub fn init(p: &Peripherals) {
    // Unlock writes to the ANCTL registers.
    p.pwr.anakey1().write(|w| unsafe { w.bits(ANAKEY1_UNLOCK) });
    p.pwr.anakey2().write(|w| unsafe { w.bits(ANAKEY2_UNLOCK) });

    // Turn off the power-on reset monitor.
    p.anctl.porcr().write(|w| unsafe { w.bits(PORCR_POR_OFF) });

    // Re-lock the ANCTL registers.
    p.pwr.anakey1().write(|w| unsafe { w.bits(ANAKEY_LOCK) });
    p.pwr.anakey2().write(|w| unsafe { w.bits(ANAKEY_LOCK) });

    set_sys_clock(p);

    // Tail of `wb32_clock_init`: enable the bus-matrix clocks. The dummy
    // read-backs mirror the `(void)RCC->APBxENR;` barriers in `rccEnableAPBx`.
    p.rcc
        .apb1enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB1ENR_BMX1EN) });
    let _ = p.rcc.apb1enr().read().bits();
    p.rcc
        .apb2enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB2ENR_BMX2EN) });
    let _ = p.rcc.apb2enr().read().bits();
}

/// Configure the main clock source, prescalers, flash wait states, HSE and PLL.
///
/// Faithful port of the vendor `SetSysClock` for the
/// HSE = 12 MHz / PLLDIV = 2 / PLLMUL = 16 / MAINCLKSRC = PLL configuration
/// with AHB/APB1/APB2 prescalers of 1.
fn set_sys_clock(p: &Peripherals) {
    // Unlock writes to the ANCTL registers.
    p.pwr.anakey1().write(|w| unsafe { w.bits(ANAKEY1_UNLOCK) });
    p.pwr.anakey2().write(|w| unsafe { w.bits(ANAKEY2_UNLOCK) });

    // APB1CLK = MAINCLK / 1: enable the source only (PPRE1 == 1, no ratio).
    // The vendor's redundant `RCC->APB1PRE |= 0x00` for the /1 case is a no-op
    // (OR with zero) and is intentionally omitted.
    p.rcc.apb1pre().write(|w| unsafe { w.bits(RCC_APB1PRE_SRCEN) });

    // Enable BMX1 and GPIOD so the HSE crystal pads (PD0/PD1) can be driven.
    p.rcc
        .apb1enr()
        .write(|w| unsafe { w.bits(RCC_APB1ENR_BMX1EN | RCC_APB1ENR_GPIODEN) });

    // Configure PD0/PD1 as analog crystal pads and start the HSE oscillator
    // (non-bypass path).
    p.gpiod.cfgmsk().write(|w| unsafe { w.bits(GPIOD_CFGMSK_HSE) });
    p.gpiod.moder().write(|w| unsafe { w.bits(GPIOD_MODER_HSE) });
    p.anctl.hsecr1().write(|w| unsafe { w.bits(ANCTL_HSECR1_PADOEN) });
    p.anctl.hsecr0().write(|w| unsafe { w.bits(ANCTL_HSECR0_HSEON) });

    // Wait until the HSE is ready, bounded so a crystal fault fails loudly over
    // RTT instead of hanging forever.
    let mut hse_timeout = HSE_STARTUP_TIMEOUT;
    while p.anctl.hsesr().read().bits() & ANCTL_HSESR_HSERDY == 0 {
        if hse_timeout == 0 {
            defmt::panic!("HSE failed to start");
        }
        hse_timeout -= 1;
    }

    // Flash prefetch + cache + 3 wait states (HCLK > 72 MHz).
    p.cache
        .cr()
        .write(|w| unsafe { w.bits(CACHE_CR_CHEEN | CACHE_CR_PREFEN_ON | CACHE_CR_LATENCY_3WS) });

    // AHBCLK = MAINCLK / 1 (HPRE == 1): for the /1 case the vendor writes
    // `RCC->AHBPRE = 0x00` directly (no ratio/DIVEN bits), performed here as-is.
    p.rcc.ahbpre().write(|w| unsafe { w.bits(RCC_AHBPRE_DIV1) });

    // APB2CLK = MAINCLK / 1: enable the source only (PPRE2 == 1, no ratio).
    // The vendor's redundant `RCC->APB2PRE |= 0x00` for the /1 case is a no-op
    // (OR with zero) and is intentionally omitted.
    p.rcc.apb2pre().write(|w| unsafe { w.bits(RCC_APB2PRE_SRCEN) });

    // PLLCLK = HSE / PLLDIV * PLLMUL = 12 MHz / 2 * 16 = 96 MHz.
    p.rcc.pllsrc().write(|w| unsafe { w.bits(RCC_PLLSRC_HSE) });
    p.rcc.pllpre().write(|w| unsafe { w.bits(RCC_PLLPRE_SRCEN) });
    // PLLDIV == 2: ratio field = PLLDIV - 2 = 0, then enable the divider.
    p.rcc
        .pllpre()
        .modify(|r, w| unsafe { w.bits(r.bits() | (PLLDIV - 2)) });
    p.rcc
        .pllpre()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_PLLPRE_DIVEN) });
    p.anctl.pllcr().write(|w| unsafe { w.bits(ANCTL_PLLCR_PLLMUL16) });

    // Enable the PLL and wait for lock, bounded like the HSE wait above.
    p.anctl.pllenr().write(|w| unsafe { w.bits(ANCTL_PLLENR_PLLON) });
    let mut pll_timeout = PLL_LOCK_TIMEOUT;
    while p.anctl.pllsr().read().bits() & ANCTL_PLLSR_READY != ANCTL_PLLSR_READY {
        if pll_timeout == 0 {
            defmt::panic!("PLL failed to lock");
        }
        pll_timeout -= 1;
    }

    // Select the PLL as the system clock and latch the change.
    p.rcc.mainclksrc().write(|w| unsafe { w.bits(RCC_MAINCLKSRC_PLL) });
    p.rcc.mainclkuen().write(|w| unsafe { w.bits(RCC_MAINCLKUEN_ENA) });

    // Re-lock the ANCTL registers.
    p.pwr.anakey1().write(|w| unsafe { w.bits(ANAKEY_LOCK) });
    p.pwr.anakey2().write(|w| unsafe { w.bits(ANAKEY_LOCK) });
}
