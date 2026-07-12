// SPDX-License-Identifier: GPL-2.0-or-later
//! Minimal GPIO port-abstraction layer for the Westberry WB32FQ95.
//!
//! The WB32 GPIO peripheral is STM32F4-style (one register file per port,
//! 2-bit `MODER`/`PUPDR`/`OSPEEDR` fields, 1-bit `OTYPER`, atomic `BSRR`),
//! with one vendor twist: the **`CFGMSK`** write-mask register. Each of the 16
//! `CFGMSK` bits guards the matching pin in the *configuration* registers
//! (`MODER`/`OTYPER`/`OSPEEDR`/`PUPDR`/`AFR*`): a pin whose mask bit is `0` is
//! writable, a pin whose mask bit is `1` is write-protected. `CFGMSK` does not
//! gate the data registers (`IDR`/`ODR`/`BSRR`). Its reset value may protect
//! pins, so every configuration write here first programs `CFGMSK` to expose
//! exactly the pin being touched and then writes the whole configuration
//! register; the mask leaves every other pin's field untouched. This mirrors
//! the HSE-pad bring-up in [`crate::clock`].
//!
//! All four ports map onto the same `RegisterBlock` layout, so a [`Port`] is
//! resolved to its memory-mapped block by address and accessed through the PAC
//! register proxies.

use pac::{gpioa::RegisterBlock, Gpioa, Gpiob, Gpioc, Gpiod, Peripherals};

/// A GPIO port.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Port {
    /// GPIOA.
    A,
    /// GPIOB.
    B,
    /// GPIOC.
    C,
    /// GPIOD. Hosts the HSE crystal pads (PD0/PD1), which are brought up by
    /// [`crate::clock`]; included here to complete the port set and so
    /// [`block`] can resolve it, though the key matrix never selects it.
    #[allow(dead_code)]
    D,
}

/// A single GPIO pin: a [`Port`] plus a pin number in `0..=15`.
#[derive(Clone, Copy)]
pub struct Pin {
    /// Owning port.
    pub port: Port,
    /// Pin number within the port (`0..=15`).
    pub num: u8,
}

impl Pin {
    /// Construct a pin from its port and number.
    pub const fn new(port: Port, num: u8) -> Self {
        Self { port, num }
    }
}

// === MODER field codes (2 bits per pin) ===
/// `MODER` digital-input mode.
const MODER_INPUT: u32 = 0b00;
/// `MODER` general-purpose-output mode.
const MODER_OUTPUT: u32 = 0b01;
/// `MODER` alternate-function mode (the pin is driven by a peripheral such as
/// UART3, selected per-pin by the `AFRL`/`AFRH` nibble).
const MODER_ALTERNATE: u32 = 0b10;

// === OTYPER field code (1 bit per pin) ===
/// `OTYPER` push-pull output stage.
const OTYPER_PUSH_PULL: u32 = 0b0;

// === OSPEEDR field code (2 bits per pin) ===
/// `OSPEEDR` highest output slew rate. Mirrors QMK's `PAL_OUTPUT_SPEED_HIGHEST`
/// used for the WB32 UART pins (`platforms/chibios/drivers/uart_serial.c:123`).
const OSPEEDR_HIGHEST: u32 = 0b11;

// === PUPDR field codes (2 bits per pin) ===
/// `PUPDR` no pull resistor (floating).
const PUPDR_NONE: u32 = 0b00;
/// `PUPDR` internal pull-up.
const PUPDR_PULL_UP: u32 = 0b01;

// === RCC.APB1ENR GPIO clock-enable bits (wb32fq95xx.h:4153-4155) ===
/// RCC.APB1ENR: GPIOA clock enable (`RCC_APB1ENR_GPIOAEN = 0x1U << 5`).
const RCC_APB1ENR_GPIOAEN: u32 = 0x1 << 5;
/// RCC.APB1ENR: GPIOB clock enable (`RCC_APB1ENR_GPIOBEN = 0x1U << 6`).
const RCC_APB1ENR_GPIOBEN: u32 = 0x1 << 6;
/// RCC.APB1ENR: GPIOC clock enable (`RCC_APB1ENR_GPIOCEN = 0x1U << 7`).
const RCC_APB1ENR_GPIOCEN: u32 = 0x1 << 7;

/// Resolve a [`Port`] to its memory-mapped register block.
#[inline]
fn block(port: Port) -> &'static RegisterBlock {
    let ptr = match port {
        Port::A => Gpioa::ptr(),
        Port::B => Gpiob::ptr(),
        Port::C => Gpioc::ptr(),
        Port::D => Gpiod::ptr(),
    };
    // SAFETY: `ptr` points at a GPIO register block that is memory-mapped at a
    // fixed address and valid for the whole program. Every access through the
    // returned reference goes via the PAC register proxies, which perform
    // volatile reads/writes; the firmware is single-threaded and no interrupt
    // handler touches these registers, so there is no aliasing hazard.
    unsafe { &*ptr }
}

/// Expose `num` (and only `num`) for the next configuration-register write by
/// clearing its `CFGMSK` bit and protecting every other pin.
#[inline]
fn expose(b: &RegisterBlock, num: u8) {
    b.cfgmsk().write(|w| unsafe { w.bits(!(1u32 << num)) });
}

/// Configure `pin` as a push-pull digital output (`MODER = 01`, `OTYPER = 0`)
/// with the pull resistor disabled (`PUPDR = 00`). Clearing the pull-up matters
/// because columns are reconfigured from [`set_input_pull_up`] on every scan; a
/// leftover pull-up would fight the push-pull driver while it holds the line
/// low, wasting current on this battery-powered board.
pub fn set_output_push_pull(pin: Pin) {
    let b = block(pin.port);
    let n = pin.num as u32;
    expose(b, pin.num);
    b.moder().write(|w| unsafe { w.bits(MODER_OUTPUT << (2 * n)) });
    b.otyper().write(|w| unsafe { w.bits(OTYPER_PUSH_PULL << n) });
    b.pupdr().write(|w| unsafe { w.bits(PUPDR_NONE << (2 * n)) });
}

/// Configure `pin` as a digital input with the internal pull-up enabled
/// (`MODER = 00`, `PUPDR = 01`).
pub fn set_input_pull_up(pin: Pin) {
    let b = block(pin.port);
    let n = pin.num as u32;
    expose(b, pin.num);
    b.moder().write(|w| unsafe { w.bits(MODER_INPUT << (2 * n)) });
    b.pupdr().write(|w| unsafe { w.bits(PUPDR_PULL_UP << (2 * n)) });
}

/// Configure `pin` as a floating digital input (`MODER = 00`, `PUPDR = 00`),
/// i.e. no internal pull resistor — the level is set entirely externally.
///
/// Mirrors QMK's `gpio_set_pin_input` (as opposed to `gpio_set_pin_input_high`,
/// which is [`set_input_pull_up`]). Used for the USB-cable-insertion sense line
/// (`HS_BAT_CABLE_PIN`, externally driven by the charger), which the akko 5075B
/// brings up floating (`ansi.c:220-222`).
pub fn set_input_floating(pin: Pin) {
    let b = block(pin.port);
    let n = pin.num as u32;
    expose(b, pin.num);
    b.moder().write(|w| unsafe { w.bits(MODER_INPUT << (2 * n)) });
    b.pupdr().write(|w| unsafe { w.bits(PUPDR_NONE << (2 * n)) });
}

/// Configure `pin` for an alternate function: `MODER = 10` (alternate), a
/// push-pull output stage, the highest slew rate, no pull resistor, and the
/// 4-bit alternate-function selector `af` programmed into the matching `AFRL`
/// (pins 0..=7) or `AFRH` (pins 8..=15) nibble.
///
/// This mirrors QMK's ChibiOS UART line setup
/// `palSetLineMode(pin, PAL_MODE_ALTERNATE(af) | PAL_OUTPUT_TYPE_PUSHPULL |
/// PAL_OUTPUT_SPEED_HIGHEST)` (`platforms/chibios/drivers/uart_serial.c:123-124`).
/// A single [`expose`] latches `CFGMSK` to this pin, after which every
/// configuration-register write below touches only `pin`'s fields.
pub fn set_alternate_push_pull(pin: Pin, af: u8) {
    let b = block(pin.port);
    let n = pin.num as u32;
    let af = (af as u32) & 0x0F;
    expose(b, pin.num);
    b.moder()
        .write(|w| unsafe { w.bits(MODER_ALTERNATE << (2 * n)) });
    b.otyper().write(|w| unsafe { w.bits(OTYPER_PUSH_PULL << n) });
    b.ospeedr()
        .write(|w| unsafe { w.bits(OSPEEDR_HIGHEST << (2 * n)) });
    b.pupdr().write(|w| unsafe { w.bits(PUPDR_NONE << (2 * n)) });
    // AFRL holds pins 0..=7, AFRH holds pins 8..=15; 4 bits per pin.
    if pin.num < 8 {
        b.afrl().write(|w| unsafe { w.bits(af << (4 * n)) });
    } else {
        b.afrh().write(|w| unsafe { w.bits(af << (4 * (n - 8))) });
    }
}

/// Drive `pin` low through the atomic bit-reset half of `BSRR`.
pub fn set_low(pin: Pin) {
    block(pin.port)
        .bsrr()
        .write(|w| unsafe { w.bits(1u32 << (pin.num as u32 + 16)) });
}

/// Drive `pin` high through the atomic bit-set half of `BSRR`.
///
/// Counterpart to [`set_low`]. The ROW2COL matrix scan never needs it (it only
/// drives the selected column low), but [`crate::rgb`] uses it to power the LED
/// rail (PA8) when RGB is enabled.
pub fn set_high(pin: Pin) {
    block(pin.port)
        .bsrr()
        .write(|w| unsafe { w.bits(1u32 << pin.num) });
}

/// Report whether `pin` reads low (logic `0`) on `IDR`.
///
/// This is the polarity the ROW2COL scan needs: a selected column is driven
/// low, so a pressed key pulls its row input low and a row that `is_low` is
/// pressed — mirroring QMK's `readMatrixPin(...) == MATRIX_INPUT_PRESSED_STATE`
/// with the default pressed state of `0`.
pub fn is_low(pin: Pin) -> bool {
    block(pin.port).idr().read().bits() & (1u32 << pin.num) == 0
}

/// Report whether `pin` reads high (logic `1`) on `IDR` — the raw-level read
/// QMK's `gpio_read_pin` returns. Used by the wireless charge-state sense lines.
pub fn is_high(pin: Pin) -> bool {
    block(pin.port).idr().read().bits() & (1u32 << pin.num) != 0
}

/// Enable the APB1 clocks for the GPIO ports the matrix uses (GPIOA/B/C).
///
/// GPIOD is already enabled by [`crate::clock`] for the HSE pads; this is a
/// read-modify-write so the BMX1/GPIOD enables stay set. The read-back is the
/// usual write barrier (mirroring the vendor `rccEnableAPB1`).
pub fn enable_clocks(p: &Peripherals) {
    p.rcc.apb1enr().modify(|r, w| unsafe {
        w.bits(r.bits() | RCC_APB1ENR_GPIOAEN | RCC_APB1ENR_GPIOBEN | RCC_APB1ENR_GPIOCEN)
    });
    let _ = p.rcc.apb1enr().read().bits();
}
