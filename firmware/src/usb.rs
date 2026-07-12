// SPDX-License-Identifier: GPL-2.0-or-later
//! USB device-controller bring-up for the Westberry WB32FQ95.
//!
//! The WB32's USB peripheral is a Mentor-MUSB ("mini" 8-bit-packed) core. The
//! USB *protocol* (endpoint state machines, control transfers, the
//! `embassy-usb-driver` implementation) is owned entirely by the [`musb`] crate
//! via its `builtin-wb32fq95` register profile; this module only supplies the
//! parts that are specific to *this* silicon:
//!
//! 1. [`wb32_usb_init`] — the clock, PHY and GPIO bring-up. This is a faithful
//!    port of the ChibiOS-Contrib vendor HAL
//!    (`os/hal/ports/WB32/WB32FQ95xx/hal_lld.c`, functions `wb32_usb_init` and
//!    `wb32_usb_connect`). Every magic bit value carries the CMSIS header line
//!    (`wb32fq95xx.h`) or `hal_lld.c` line it derives from, exactly as
//!    [`crate::clock`] and [`crate::time_driver`] do.
//! 2. [`Driver`] — a thin adapter that exposes [`musb::MusbDriver`] as an
//!    [`embassy_usb_driver::Driver`].
//! 3. The [`USB`] interrupt handler, which forwards to [`musb::on_interrupt`].
//! 4. [`usb_task`] — an `embassy-usb` device exposing three HID interfaces: the
//!    6KRO boot keyboard (driven by [`keyboard_loop`], which scans the matrix, runs
//!    the [`crate::keymap`] engine and feeds the boot keyboard IN endpoint), a
//!    shared report-ID interface carrying NKRO + consumer + system control + mouse +
//!    gamepad (driven by [`shared_ep_loop`]) and the raw-HID vendor interface that carries the
//!    [`crate::kcp`] config protocol (driven by [`kcp_loop`]). The device and the
//!    three loops run concurrently on this one task via [`join4`].
//!
//! # Division of labour with `musb`
//!
//! `wb32_usb_init` brings up everything through the PA11/PA12 D-/D+ pads and
//! sets the MUSB `POWER.Enable_Suspend_M` bit (suspend detection), matching the
//! vendor `wb32_usb_connect`. It deliberately leaves the `INTRUSBE`/`INTRTXE`/
//! `INTRRXE` interrupt-enable registers to `musb`, which programs them from its
//! `bus_init` (run when the bus is first polled, after [`Driver::start`]). On
//! this "mini" core the `POWER` register has no soft-connect bit, so the device
//! is connected purely by configuring PA11/PA12 as USB alternate-function pads
//! (and disconnected by driving D+ low) — there is no separate MUSB pull-up
//! register to poke here.

use crate::{
    digitizer, features, gamepad, gpio, kcp, keymap, matrix, midi, mouse, telemetry, wireless,
    xinput,
};
use core::cell::RefCell;
use core::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU8, Ordering};
use embassy_futures::join::{join3, join4};
use embassy_futures::select::{select, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::signal::Signal;
use embassy_time::{Duration, Instant, Timer};
use embassy_usb::class::hid::{self, HidReader, HidReaderWriter, HidWriter, State};
use embassy_usb::class::midi::MidiClass;
use embassy_usb::{Builder, Config, Handler};
use embassy_usb_driver::{EndpointAddress, EndpointAllocError, EndpointType};
use musb::{In, MusbDriver, MusbInstance, Out, UsbInstance};
use usbd_hid::descriptor::{KeyboardReport, SerializedDescriptor};
use pac::{Interrupt, Peripherals};

// ===========================================================================
// USB clock / PHY / GPIO bring-up (the WB32-specific shim)
// ===========================================================================

// === RCC.AHBENR1: AHB peripheral clock enables ===
/// RCC.AHBENR1: CRC/SFM clock enable. Must be set before writing the SFM USB
/// PHY control register (`RCC_AHBENR1_CRCSFMEN = 0x1U << 8`, wb32fq95xx.h:4142).
const RCC_AHBENR1_CRCSFMEN: u32 = 0x1 << 8;
/// RCC.AHBENR1: USB peripheral clock enable
/// (`RCC_AHBENR1_USBEN = 0x1U << 1`, wb32fq95xx.h:4135).
const RCC_AHBENR1_USBEN: u32 = 0x1 << 1;

// === RCC.AHBRSTR1: AHB peripheral resets ===
/// RCC.AHBRSTR1: USB peripheral reset, pulsed to reset the controller
/// (`RCC_AHBRSTR1_USBRST = 0x1U << 1`, wb32fq95xx.h:4203).
const RCC_AHBRSTR1_USBRST: u32 = 0x1 << 1;

// === RCC USB FIFO clock ===
/// RCC.USBFIFOCLKSRC: select USBCLK as the FIFO clock source
/// (`RCC_USBFIFOCLKSRC_USBCLK = 0x1U`, wb32fq95xx.h:4121).
const RCC_USBFIFOCLKSRC_USBCLK: u32 = 0x1;
/// RCC.USBFIFOCLKENR: enable the USB FIFO clock
/// (`RCC_USBFIFOCLKENR_CLKEN = 0x1U`, wb32fq95xx.h:4200).
const RCC_USBFIFOCLKENR_CLKEN: u32 = 0x1;

// === SFM.USBPCON: USB PHY port control ===
/// SFM.USBPCON value that configures and enables the USB transceiver (PHY).
/// The vendor HAL writes the literal `0x02` (`SFM->USBPCON = 0x02`,
/// hal_lld.c:327); the CMSIS header declares only the register, not the field,
/// so this mirrors the vendor value verbatim.
const SFM_USBPCON_PHY_ENABLE: u32 = 0x02;

// === RCC USB clock (USBCLK = main clock / 2 = 48 MHz) ===
/// RCC.USBCLKENR: enable USBCLK
/// (`RCC_USBCLKENR_CLKEN = 0x1U`, wb32fq95xx.h:4188).
const RCC_USBCLKENR_CLKEN: u32 = 0x1;
/// RCC.USBPRE: enable the USB prescaler source
/// (`RCC_USBPRE_SRCEN = 0x1U << 3`, wb32fq95xx.h:3317).
const RCC_USBPRE_SRCEN: u32 = 0x1 << 3;
/// RCC.USBPRE: ratio = /2, which the field encodes as zero
/// (`RCC_USBPRE_RATIO_2 = 0x0U << 1`, wb32fq95xx.h:3314). USB requires exactly
/// 48 MHz, obtained as the 96 MHz main clock divided by two.
const RCC_USBPRE_RATIO_2: u32 = 0x0 << 1;
/// RCC.USBPRE: enable the prescaler divider
/// (`RCC_USBPRE_DIVEN = 0x1U << 0`, wb32fq95xx.h:3310).
const RCC_USBPRE_DIVEN: u32 = 0x1 << 0;

// === RCC.APB1ENR: bus-matrix / GPIOA clock enables ===
/// RCC.APB1ENR: bus-matrix BMX1 clock enable
/// (`RCC_APB1ENR_BMX1EN = 0x1U << 15`, wb32fq95xx.h:4163).
const RCC_APB1ENR_BMX1EN: u32 = 0x1 << 15;
/// RCC.APB1ENR: GPIOA clock enable; PA11/PA12 carry USB D-/D+
/// (`RCC_APB1ENR_GPIOAEN = 0x1U << 5`, wb32fq95xx.h:4153).
const RCC_APB1ENR_GPIOAEN: u32 = 0x1 << 5;

// === GPIOA.CFGMSK: per-pin write-mask exposing PA11/PA12 ===
// See [`crate::gpio`] for the CFGMSK semantics: a mask bit of 0 makes the pin
// writable in the configuration registers, a mask bit of 1 protects it.
/// GPIO.CFGMSK bit guarding PA11 (`GPIO_CFGMSK_CFGMSK11 = 0x1U << 11`,
/// wb32fq95xx.h:1330).
const GPIO_CFGMSK_CFGMSK11: u32 = 0x1 << 11;
/// GPIO.CFGMSK bit guarding PA12 (`GPIO_CFGMSK_CFGMSK12 = 0x1U << 12`,
/// wb32fq95xx.h:1331).
const GPIO_CFGMSK_CFGMSK12: u32 = 0x1 << 12;

// === GPIOA PA11/PA12 alternate-function configuration (USB D-/D+) ===
// The following three values are positional, derived from the vendor
// `wb32_usb_connect` (hal_lld.c:391-397). `MODER`/`CURRENT` are two bits per
// pin (pin 11 -> bits 23:22, pin 12 -> bits 25:24); `AFRH` is four bits per pin
// for pins 8..15 (pin 11 -> bits 15:12, pin 12 -> bits 19:16).
/// GPIOA.CURRENT: maximum drive current (`0b11`) on PA11/PA12
/// (hal_lld.c:391, `(0x3 << 22) | (0x3 << 24)`).
const GPIOA_CURRENT_USB: u32 = (0x3 << 22) | (0x3 << 24);
/// GPIOA.MODER: alternate-function mode (`0b10`) on PA11/PA12
/// (hal_lld.c:393, `(0x2 << 22) | (0x2 << 24)`).
const GPIOA_MODER_USB_AF: u32 = (0x2 << 22) | (0x2 << 24);
/// GPIOA.AFRH: AF3 selection for PA11(D-)/PA12(D+)
/// (hal_lld.c:397, `(3 << 12) | (3 << 16)`).
const GPIOA_AFRH_USB_AF3: u32 = (3 << 12) | (3 << 16);

/// Bring up the WB32FQ95 USB controller: clocks, PHY, and the D-/D+ pads.
///
/// Faithful port of the vendor `wb32_usb_init` followed by `wb32_usb_connect`
/// (ChibiOS-Contrib `hal_lld.c`), for the single USB instance, with two
/// deliberate omissions that [`musb`] owns instead:
///
/// * The `INTRUSBE`/`INTRTXE`/`INTRRXE` interrupt-enable programming from
///   `wb32_usb_connect` (hal_lld.c:400) is left to `musb`'s `bus_init`, which
///   sets the reset/suspend/resume enables when the bus is first polled. The
///   `POWER.Enable_Suspend_M` bit (hal_lld.c:399) is set here, at connect.
/// * Soft-connect: this mini core's `POWER` register has no soft-connect bit,
///   so configuring PA11/PA12 as USB alternate-function pads *is* the connect.
///
/// Must be called after [`clock::init`](crate::clock::init) (it needs the
/// 96 MHz main clock for the /2 USB clock) and before the [`usb_task`] runs.
///
// Known limitation (USB suspend): the vendor USB LLD re-applies `POWER` after
// every USB bus reset (hal_usb_lld.c:615); `musb`'s embassy reset path does not.
// If the WB32 MUSB clears `Enable_Suspend_M` on a host-issued bus reset, the
// connect-time write below is lost after that reset, so host-driven USB suspend is
// not relied on. Restoring it would mean having `musb`'s reset handler re-apply the
// bit; the keystroke and charge paths are unaffected, so the firmware ships as-is.
pub fn wb32_usb_init(p: &Peripherals) {
    // --- wb32_usb_init: clocks + PHY (hal_lld.c:306-354) ---

    // Enable the CRC/SFM clock first: the USB PHY control register lives in the
    // SFM block and is unreachable until this clock is running.
    p.rcc
        .ahbenr1()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_AHBENR1_CRCSFMEN) });

    // Enable the USB peripheral clock.
    p.rcc
        .ahbenr1()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_AHBENR1_USBEN) });

    // Pulse the USB peripheral reset.
    p.rcc
        .ahbrstr1()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_AHBRSTR1_USBRST) });
    p.rcc
        .ahbrstr1()
        .modify(|r, w| unsafe { w.bits(r.bits() & !RCC_AHBRSTR1_USBRST) });

    // Select and enable the USB FIFO clock.
    p.rcc
        .usbfifoclksrc()
        .write(|w| unsafe { w.bits(RCC_USBFIFOCLKSRC_USBCLK) });
    p.rcc
        .usbfifoclkenr()
        .write(|w| unsafe { w.bits(RCC_USBFIFOCLKENR_CLKEN) });

    // Configure and enable the USB PHY (needs the CRC/SFM clock from above).
    p.sfm
        .usbpcon()
        .write(|w| unsafe { w.bits(SFM_USBPCON_PHY_ENABLE) });

    // Enable USBCLK and set the prescaler to /2 (48 MHz from the 96 MHz main
    // clock). RATIO_2 encodes the /2 ratio as zero, composed here in one write;
    // the vendor splits it across `= SRCEN; |= RATIO_2; |= DIVEN`.
    p.rcc
        .usbclkenr()
        .write(|w| unsafe { w.bits(RCC_USBCLKENR_CLKEN) });
    p.rcc
        .usbpre()
        .write(|w| unsafe { w.bits(RCC_USBPRE_SRCEN | RCC_USBPRE_RATIO_2 | RCC_USBPRE_DIVEN) });

    // --- wb32_usb_connect: PA11(D-)/PA12(D+) as USB AF3 (hal_lld.c:384-401) ---

    // Enable the BMX1 and GPIOA clocks (read-modify-write so the clocks the
    // matrix/clock bring-up already enabled stay on); the read-back is the
    // usual write barrier.
    p.rcc
        .apb1enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB1ENR_BMX1EN | RCC_APB1ENR_GPIOAEN) });
    let _ = p.rcc.apb1enr().read().bits();

    // Expose only PA11/PA12 for configuration; CFGMSK protects every other PA
    // pin, so the whole-register writes below only land on 11/12. This is why
    // the order relative to the matrix's PA-pin setup does not matter.
    p.gpioa
        .cfgmsk()
        .write(|w| unsafe { w.bits(!(GPIO_CFGMSK_CFGMSK11 | GPIO_CFGMSK_CFGMSK12)) });
    p.gpioa
        .current()
        .write(|w| unsafe { w.bits(GPIOA_CURRENT_USB) });
    p.gpioa
        .moder()
        .write(|w| unsafe { w.bits(GPIOA_MODER_USB_AF) });
    // Push-pull, no extra speed/pull settings (the vendor writes zeros here);
    // CFGMSK keeps these confined to PA11/PA12.
    p.gpioa.otyper().write(|w| unsafe { w.bits(0) });
    p.gpioa.ospeedr().write(|w| unsafe { w.bits(0) });
    p.gpioa.pupdr().write(|w| unsafe { w.bits(0) });
    p.gpioa
        .afrh()
        .write(|w| unsafe { w.bits(GPIOA_AFRH_USB_AF3) });

    // Enable MUSB suspend detection (`POWER.Enable_Suspend_M`), mirroring the
    // vendor `wb32_usb_connect` (`USB->POWER = USB_POWER_SUSEN`, hal_lld.c:399;
    // `USB_POWER_SUSEN = 0x1U << 0`, wb32fq95xx.h:2015). The WB32 USB peripheral
    // is owned by `musb`, not the PAC, so this is written through `musb`'s
    // generated register block. The USB clocks are up by now (above), so the
    // register is writable; `musb`'s `start`/`bus_init` only program
    // `INTRUSBE`/`INTRTXE`/`INTRRXE` afterward and never touch `POWER`, so this
    // setting persists into operation. (At connect `POWER` is at its reset
    // value, so this read-modify-write equals the vendor's plain assignment.)
    UsbInstance::regs()
        .power()
        .modify(|w| w.set_enable_suspend_m(true));

    // Route the USB controller interrupt (IRQ 14) through the NVIC. `musb`'s
    // `on_interrupt` (see [`USB`]) drives all device events from this line.
    cortex_m::peripheral::NVIC::unpend(Interrupt::USB);
    // SAFETY: enabling the USB interrupt cannot break any mask-based critical
    // section; the handler only touches USB registers and atomics.
    unsafe { cortex_m::peripheral::NVIC::unmask(Interrupt::USB) };
}

// ===========================================================================
// embassy-usb-driver adapter over musb
// ===========================================================================

/// Adapts [`musb::MusbDriver`] to the [`embassy_usb_driver::Driver`] trait.
///
/// In this revision of `musb`, `MusbDriver` provides endpoint allocation and
/// `start`, but its `embassy_usb_driver::Driver` impl is left commented out
/// upstream. This wrapper supplies that impl, forwarding endpoint allocation to
/// [`MusbDriver::alloc_endpoint`] (selecting direction with `musb`'s
/// [`In`]/[`Out`] marker types) and `start` straight through.
pub struct Driver<'d> {
    inner: MusbDriver<'d, UsbInstance>,
}

impl<'d> Driver<'d> {
    /// Create the MUSB-backed driver. [`wb32_usb_init`] must already have
    /// clocked and connected the controller.
    pub fn new() -> Self {
        Self {
            inner: MusbDriver::new(),
        }
    }
}

impl<'d> embassy_usb_driver::Driver<'d> for Driver<'d> {
    type EndpointOut = musb::Endpoint<'d, UsbInstance, Out>;
    type EndpointIn = musb::Endpoint<'d, UsbInstance, In>;
    type ControlPipe = musb::ControlPipe<'d, UsbInstance>;
    type Bus = musb::Bus<'d, UsbInstance>;

    fn alloc_endpoint_out(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointOut, EndpointAllocError> {
        self.inner
            .alloc_endpoint::<Out>(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn alloc_endpoint_in(
        &mut self,
        ep_type: EndpointType,
        ep_addr: Option<EndpointAddress>,
        max_packet_size: u16,
        interval_ms: u8,
    ) -> Result<Self::EndpointIn, EndpointAllocError> {
        self.inner
            .alloc_endpoint::<In>(ep_type, ep_addr, max_packet_size, interval_ms)
    }

    fn start(self, control_max_packet_size: u16) -> (Self::Bus, Self::ControlPipe) {
        self.inner.start(control_max_packet_size)
    }
}

/// USB global interrupt (IRQ 14).
///
/// The PAC does not re-export cortex-m-rt's `#[interrupt]` attribute, so the
/// handler is provided directly as the `USB` symbol, overriding the weak
/// `PROVIDE(USB = DefaultHandler)` from the PAC's `device.x` — the same pattern
/// [`crate::time_driver`] uses for `TIM2`. It forwards to `musb`, which decodes
/// the MUSB interrupt-status registers and wakes the relevant device futures.
#[no_mangle]
extern "C" fn USB() {
    // SAFETY: invoked only from the USB interrupt. `UsbInstance` is the one and
    // only MUSB controller; `on_interrupt` reads its interrupt registers and
    // signals wakers, touching no state owned outside the driver.
    unsafe { musb::on_interrupt::<UsbInstance>() };
}

// ===========================================================================
// Minimal embassy-usb HID device
// ===========================================================================

// USB identity: the pid.codes open-source TEST allocation (VID 0x1209 / PID
// 0x0001). This is the firmware's shipping identity by decision; obtaining a
// production VID/PID is a distribution-time concern, not a firmware one.
/// USB vendor ID (pid.codes open-source TEST space). Also advertised to the
/// 2.4G dongle by [`crate::wireless`] so its USB identity matches the keyboard's.
pub(crate) const VID: u16 = 0x1209;
/// USB product ID (pid.codes generic TEST PID). See [`VID`].
pub(crate) const PID: u16 = 0x0001;

/// USB manufacturer string — the one brand presented on every link: the wired
/// keyboard and MIDI personality here, and (via [`crate::wireless`]) the 2.4G
/// dongle's pushed manufacturer string. One physical board, one manufacturer. The
/// per-mode *product* names live in [`crate::wireless::Devs::device_name`]; this is
/// the shared manufacturer they all share, alongside [`VID`]/[`PID`].
pub(crate) const MANUFACTURER: &str = "Akko";

/// Whether the host has the device configured (a non-zero `SET_CONFIGURATION`
/// is in effect). The report loops emit only while this is set, so they never
/// write into an IN endpoint the host cannot drain. Maintained by
/// [`UsbStateHandler`]; read from [`keyboard_loop`] and [`shared_ep_loop`].
static CONFIGURED: AtomicBool = AtomicBool::new(false);

/// Whether a USB host has configured the device — the "a real USB data host is
/// present" signal the transport supervisor
/// ([`crate::wireless::transport_supervisor_task`]) reads to distinguish a wired
/// host from a charge-only cable (VBUS but no enumeration).
///
/// The MUSB core does not report VBUS removal (its embassy driver leaves that a
/// `// TODO`), so this stays set after an unplug; the supervisor therefore pairs it
/// with the hardware cable-sense pin, which supplies the authoritative plug/unplug
/// edge. A brief VBUS glitch never satisfies this (no enumeration), so it also keeps
/// a wobbling cable from stealing the session from a working wireless link.
pub(crate) fn usb_configured() -> bool {
    CONFIGURED.load(Ordering::Relaxed)
}

/// Force the configured flag low. The transport supervisor
/// ([`crate::wireless::transport_supervisor_task`]) calls this whenever the hardware
/// cable-sense pin reads no cable, so a `CONFIGURED` left stale-true by an unplug (the
/// MUSB core reports no VBUS-off) cannot make a later charge-only reinsert — VBUS but
/// no host — falsely read as a wired host. A real host re-sets it via
/// `SET_CONFIGURATION` on the next enumeration.
pub(crate) fn clear_configured() {
    CONFIGURED.store(false, Ordering::Relaxed);
}

/// The consumer-control usage the keymap engine currently resolves (`0` = no
/// media key held), published by [`keyboard_loop`] every scan and consumed by
/// [`shared_ep_loop`].
///
/// This decouples the two: [`keyboard_loop`] resolves the usage from the same
/// debounced scan it builds the keyboard report from, but the actual consumer
/// HID write — whose `await` can block when the host is slow to poll the
/// shared endpoint — happens in [`shared_ep_loop`], so it can never stall
/// matrix scanning or the keyboard report path. A relaxed `u16` store/load is
/// all the ordering this single-producer/single-consumer hand-off needs.
static CONSUMER_USAGE: AtomicU16 = AtomicU16::new(0);

/// The set of mouse keys the keymap engine currently resolves as held (`0` = none),
/// as the [`crate::mouse`] bitmask. Published by [`keyboard_loop`] every scan and
/// consumed by [`shared_ep_loop`], which feeds it to the [`mouse::Accel`]
/// accelerator and sends the mouse HID report (report 4 on the shared interface).
///
/// Decoupled exactly like [`CONSUMER_USAGE`]: the bitmask is level state, so the
/// (potentially host-blocked) EP3 mouse write happens in [`shared_ep_loop`], never
/// on this matrix-scanning path. A relaxed `u16` store/load is all the ordering this
/// single-producer/single-consumer hand-off needs.
static MOUSE_KEYS: AtomicU16 = AtomicU16::new(0);

/// The set of gamepad keys the keymap engine currently resolves as held (`0` =
/// none), as the [`crate::gamepad`] bitmask (buttons in the low 16 bits, axis flags
/// above). Published by [`keyboard_loop`] every scan and consumed by
/// [`shared_ep_loop`], which decodes it into the gamepad HID report (report 5 on the
/// shared interface).
///
/// Decoupled exactly like [`MOUSE_KEYS`]: the bitmask is level state, so the
/// (potentially host-blocked) EP3 gamepad write happens in [`shared_ep_loop`], never
/// on this matrix-scanning path. A relaxed `u32` store/load is all the ordering this
/// single-producer/single-consumer hand-off needs.
static GAMEPAD_KEYS: AtomicU32 = AtomicU32::new(0);

/// Whether full N-key rollover is enabled (the kcp `HID_KRO` toggle).
///
/// Default `false` — boot 6-key rollover — so a freshly powered keyboard sends only
/// the BIOS-compatible 6KRO boot report. The host flips it live over kcp
/// ([`set_nkro`]); it is
/// read every scan by [`keyboard_loop`] to decide whether to dual-send the NKRO
/// bitmap alongside the 6KRO boot report. RAM-live and persisted as part of the
/// CONFIG flash blob (a host `CONFIG.SAVE` stores it; restored at boot, else the
/// default).
static NKRO_ENABLED: AtomicBool = AtomicBool::new(false);

/// Whether full N-key rollover is enabled. Read by [`keyboard_loop`]; exposed for
/// the kcp `HID_KRO` group ([`kcp`]).
pub(crate) fn nkro_enabled() -> bool {
    NKRO_ENABLED.load(Ordering::Relaxed)
}

/// Set the rollover mode (the kcp `HID_KRO` `SET_KRO` op). Applied live on the next
/// [`keyboard_loop`] scan.
pub(crate) fn set_nkro(on: bool) {
    NKRO_ENABLED.store(on, Ordering::Relaxed);
}

/// The NKRO "overflow" half of the dual-send, published by [`keyboard_loop`] every
/// scan (under USB) and consumed by [`shared_ep_loop`].
///
/// When NKRO is enabled the keyboard loop splits the held set so the lowest six
/// usages ride the 6KRO boot report (EP1) and the rest are carried as a bitmap;
/// `mods` is the shared modifier byte and `bits` that overflow bitmap. The split is
/// done where the report is built (so EP1 and the EP3 NKRO report stay disjoint —
/// no double-typing) and handed to [`shared_ep_loop`] here so the EP3 write, which
/// can block on a slow host, never stalls matrix scanning or the EP1 boot path —
/// the same decoupling as [`CONSUMER_USAGE`]. When NKRO is disabled the loop
/// publishes the idle value (`mods = 0`, empty `bits`), so [`shared_ep_loop`]
/// releases the NKRO report and then goes quiescent.
struct NkroOut {
    mods: u8,
    bits: [u8; keymap::NKRO_BYTES],
}

/// Published [`NkroOut`]. A blocking critical-section mutex with an inner `RefCell`,
/// like the keymap/behaviour stores: every access is a synchronous copy with no
/// `.await` held, and the producer ([`keyboard_loop`]) and consumer
/// ([`shared_ep_loop`]) are futures on the one cooperative executor, so the borrow
/// is never re-entrant.
static NKRO_OUT: Mutex<CriticalSectionRawMutex, RefCell<NkroOut>> = Mutex::const_new(
    CriticalSectionRawMutex::new(),
    RefCell::new(NkroOut {
        mods: 0,
        bits: [0; keymap::NKRO_BYTES],
    }),
);

/// Publish the current NKRO overflow (modifier + bitmap) for [`shared_ep_loop`].
fn publish_nkro(mods: u8, bits: [u8; keymap::NKRO_BYTES]) {
    NKRO_OUT.lock(|cell| *cell.borrow_mut() = NkroOut { mods, bits });
}

/// Read the latest published NKRO overflow.
fn load_nkro() -> (u8, [u8; keymap::NKRO_BYTES]) {
    NKRO_OUT.lock(|cell| {
        let o = cell.borrow();
        (o.mods, o.bits)
    })
}

/// Device event handler mirroring the host's configuration state into
/// [`CONFIGURED`].
///
/// A bus reset returns the device to the unconfigured (Default) state until the
/// host re-issues `SET_CONFIGURATION`, so [`Handler::reset`] clears the flag as
/// well; [`Handler::configured`] then sets it on the following configuration.
struct UsbStateHandler;

impl Handler for UsbStateHandler {
    fn configured(&mut self, configured: bool) {
        CONFIGURED.store(configured, Ordering::Relaxed);
    }

    fn reset(&mut self) {
        CONFIGURED.store(false, Ordering::Relaxed);
    }
}

/// Raw-HID report descriptor for the [`crate::kcp`] vendor interface.
///
/// This is the standard QMK `raw_hid` descriptor: a vendor-defined collection on
/// usage page `0xFF60`, usage `0x61`, with one 32-byte Input report (usage
/// `0x62`) and one 32-byte Output report (usage `0x63`), each an array of bytes
/// (logical range 0..255). The usage page is chosen *only* because the stock
/// 2.4 GHz dongle bridges exactly this page, so the same interface reaches the
/// host over USB and, through the 2.4G dongle bridge, over the radio. There is no
/// report ID, so each HID report is exactly one 32-byte [`kcp`] message.
const KCP_HID_DESCRIPTOR: &[u8] = &[
    0x06, 0x60, 0xFF, // Usage Page (Vendor 0xFF60)
    0x09, 0x61, //       Usage (0x61)
    0xA1, 0x01, //       Collection (Application)
    0x09, 0x62, //         Usage (0x62)
    0x15, 0x00, //         Logical Minimum (0)
    0x26, 0xFF, 0x00, //   Logical Maximum (255)
    0x95, 0x20, //         Report Count (32)
    0x75, 0x08, //         Report Size (8)
    0x81, 0x02, //         Input (Data,Var,Abs)
    0x09, 0x63, //         Usage (0x63)
    0x91, 0x02, //         Output (Data,Var,Abs)
    0xC0, //             End Collection
];

/// HID report size (bytes) of the raw-HID endpoints, i.e. one [`kcp`] message.
/// Used for both the IN and OUT reports. Tied to [`kcp::MSG_LEN`] by the
/// assertion below so the descriptor's `Report Count (32)` and the endpoint
/// `max_packet_size` cannot drift from the protocol's frame length.
const KCP_REPORT_LEN: usize = 32;
const _: () = assert!(
    KCP_REPORT_LEN == kcp::MSG_LEN,
    "raw-HID report size must equal the kcp message length"
);

// === Shared report-ID HID interface (NKRO + consumer + system + mouse + gamepad, EP3) ===
//
// One interface, one IN endpoint (EP3), five report IDs — the budget-aware way to
// carry NKRO, consumer, system control, the mouse and the gamepad without a fourth
// IN endpoint the MUSB core does not have. Each report is prefixed with its report
// ID on the wire, so the single [`HidWriter`] carries all five. This is a
// hand-written descriptor (like [`KCP_HID_DESCRIPTOR`]) for precise control over the
// NKRO bitmap width and because `usbd-hid`'s derive emits no serializer once report
// IDs are present — the loops build the bytes directly.
//
// Report 1 (NKRO keyboard): a modifier byte (usages 0xE0..0xE7) + a 112-bit bitmap
// over usages 0x00..0x6F ([`keymap::NKRO_BYTES`] bytes), the standard QMK NKRO
// shape. Report 2 (consumer): one 16-bit consumer usage (the old standalone
// `MediaKeyboardReport`, now report-ID-prefixed). Report 3 (system control): one
// 8-bit System Control usage (0x81..0xB7). Report 4 (mouse): a button byte (3
// buttons + padding) and three signed bytes — relative X, Y and wheel — the
// standard boot-mouse shape, fed by the [`mouse::Accel`] accelerator. Report 5
// (gamepad): a 16-bit button field and four signed, *absolute* axis bytes (X, Y, Z,
// Rz), decoded from the held gamepad keys by [`gamepad::buttons`]/[`gamepad::axes`].

/// NKRO keyboard report ID (report 1 on the shared interface).
const REPORT_ID_NKRO: u8 = 1;
/// Consumer-control report ID (report 2).
const REPORT_ID_CONSUMER: u8 = 2;
/// System-control report ID (report 3).
const REPORT_ID_SYSTEM: u8 = 3;
/// Mouse report ID (report 4).
const REPORT_ID_MOUSE: u8 = 4;
/// Gamepad report ID (report 5).
const REPORT_ID_GAMEPAD: u8 = 5;

/// Hand-written report descriptor for the shared EP3 interface: five Application
/// collections, one per report ID (NKRO keyboard, consumer, system control, mouse,
/// gamepad). The NKRO bitmap width is [`keymap::NKRO_BYTES`] (112 usages,
/// `0x00..=0x6F`); the
/// `Report Count` byte below (`0x70` = 112) is tied to it by the assertion that
/// follows so the descriptor and the bitmap builder cannot drift.
const SHARED_HID_DESCRIPTOR: &[u8] = &[
    // ---- Report 1: NKRO keyboard ----
    0x05, 0x01, //       Usage Page (Generic Desktop)
    0x09, 0x06, //       Usage (Keyboard)
    0xA1, 0x01, //       Collection (Application)
    0x85, REPORT_ID_NKRO, //   Report ID (1)
    //   Modifier byte: 8 bits, one per usage 0xE0..0xE7.
    0x05, 0x07, //         Usage Page (Keyboard/Keypad)
    0x19, 0xE0, //         Usage Minimum (0xE0, Left Control)
    0x29, 0xE7, //         Usage Maximum (0xE7, Right GUI)
    0x15, 0x00, //         Logical Minimum (0)
    0x25, 0x01, //         Logical Maximum (1)
    0x75, 0x01, //         Report Size (1)
    0x95, 0x08, //         Report Count (8)
    0x81, 0x02, //         Input (Data,Var,Abs)
    //   Key bitmap: 112 bits, one per usage 0x00..0x6F.
    0x05, 0x07, //         Usage Page (Keyboard/Keypad)
    0x19, 0x00, //         Usage Minimum (0x00)
    0x29, 0x6F, //         Usage Maximum (0x6F)
    0x15, 0x00, //         Logical Minimum (0)
    0x25, 0x01, //         Logical Maximum (1)
    0x75, 0x01, //         Report Size (1)
    0x95, 0x70, //         Report Count (112 = NKRO_BYTES * 8)
    0x81, 0x02, //         Input (Data,Var,Abs)
    0xC0, //             End Collection
    // ---- Report 2: consumer control ----
    0x05, 0x0C, //       Usage Page (Consumer)
    0x09, 0x01, //       Usage (Consumer Control)
    0xA1, 0x01, //       Collection (Application)
    0x85, REPORT_ID_CONSUMER, // Report ID (2)
    0x15, 0x00, //         Logical Minimum (0)
    0x26, 0x14, 0x05, //   Logical Maximum (0x514)
    0x19, 0x00, //         Usage Minimum (0)
    0x2A, 0x14, 0x05, //   Usage Maximum (0x514)
    0x75, 0x10, //         Report Size (16)
    0x95, 0x01, //         Report Count (1)
    0x81, 0x00, //         Input (Data,Array,Abs)
    0xC0, //             End Collection
    // ---- Report 3: system control ----
    0x05, 0x01, //       Usage Page (Generic Desktop)
    0x09, 0x80, //       Usage (System Control)
    0xA1, 0x01, //       Collection (Application)
    0x85, REPORT_ID_SYSTEM, //  Report ID (3)
    0x15, 0x01, //         Logical Minimum (1, macOS scrollbar compatibility)
    // Logical Maximum must be a 2-byte item: HID logical min/max are signed, so the
    // 1-byte form `0x25, 0xB7` would decode as -73 (< Logical Minimum) and a strict
    // host could reject report 3 or the whole shared interface. `0x26, 0xB7, 0x00`
    // is +183, covering the 0x81..0xB7 system usage range.
    0x26, 0xB7, 0x00, //   Logical Maximum (0xB7)
    0x19, 0x81, //         Usage Minimum (0x81, System Power Down)
    0x29, 0xB7, //         Usage Maximum (0xB7)
    0x75, 0x08, //         Report Size (8)
    0x95, 0x01, //         Report Count (1)
    0x81, 0x00, //         Input (Data,Array,Abs)
    0xC0, //             End Collection
    // ---- Report 4: mouse (buttons + relative X/Y/wheel) ----
    0x05, 0x01, //       Usage Page (Generic Desktop)
    0x09, 0x02, //       Usage (Mouse)
    0xA1, 0x01, //       Collection (Application)
    0x85, REPORT_ID_MOUSE, //  Report ID (4)
    0x09, 0x01, //         Usage (Pointer)
    0xA1, 0x00, //         Collection (Physical)
    //   Buttons: 3 bits, one per button 1..3.
    0x05, 0x09, //           Usage Page (Button)
    0x19, 0x01, //           Usage Minimum (Button 1)
    0x29, 0x03, //           Usage Maximum (Button 3)
    0x15, 0x00, //           Logical Minimum (0)
    0x25, 0x01, //           Logical Maximum (1)
    0x75, 0x01, //           Report Size (1)
    0x95, 0x03, //           Report Count (3)
    0x81, 0x02, //           Input (Data,Var,Abs)
    //   5 padding bits to byte-align the button field.
    0x75, 0x05, //           Report Size (5)
    0x95, 0x01, //           Report Count (1)
    0x81, 0x03, //           Input (Const,Var,Abs)
    //   Relative X, Y and wheel: three signed bytes (-127..127).
    0x05, 0x01, //           Usage Page (Generic Desktop)
    0x09, 0x30, //           Usage (X)
    0x09, 0x31, //           Usage (Y)
    0x09, 0x38, //           Usage (Wheel)
    0x15, 0x81, //           Logical Minimum (-127)
    0x25, 0x7F, //           Logical Maximum (127)
    0x75, 0x08, //           Report Size (8)
    0x95, 0x03, //           Report Count (3)
    0x81, 0x06, //           Input (Data,Var,Rel)
    0xC0, //             End Collection (Physical)
    0xC0, //             End Collection (Application)
    // ---- Report 5: gamepad (16 buttons + 4 signed axes) ----
    0x05, 0x01, //       Usage Page (Generic Desktop)
    0x09, 0x05, //       Usage (Game Pad)
    0xA1, 0x01, //       Collection (Application)
    0x85, REPORT_ID_GAMEPAD, // Report ID (5)
    //   Buttons: 16 bits, one per button 1..16.
    0x05, 0x09, //         Usage Page (Button)
    0x19, 0x01, //         Usage Minimum (Button 1)
    0x29, 0x10, //         Usage Maximum (Button 16)
    0x15, 0x00, //         Logical Minimum (0)
    0x25, 0x01, //         Logical Maximum (1)
    0x75, 0x01, //         Report Size (1)
    0x95, 0x10, //         Report Count (16)
    0x81, 0x02, //         Input (Data,Var,Abs)
    //   Axes: X, Y, Z and Rz — four signed, absolute bytes (-127..127). Absolute
    //   (not Rel like the mouse): an axis report states the stick position, so a
    //   held direction key pins the axis to full deflection and release re-centres.
    0x05, 0x01, //         Usage Page (Generic Desktop)
    0x09, 0x30, //         Usage (X)
    0x09, 0x31, //         Usage (Y)
    0x09, 0x32, //         Usage (Z)
    0x09, 0x35, //         Usage (Rz)
    0x15, 0x81, //         Logical Minimum (-127)
    0x25, 0x7F, //         Logical Maximum (127)
    0x75, 0x08, //         Report Size (8)
    0x95, 0x04, //         Report Count (4)
    0x81, 0x02, //         Input (Data,Var,Abs)
    0xC0, //             End Collection (Application)
    // ---- Report 6: digitizer (absolute single-touch pointer) ----
    // A host-driven absolute pointer (set over kcp, not the keymap). Tip switch +
    // in-range as two 1-bit flags (6 padding bits to a byte), then absolute X/Y as
    // unsigned 16-bit over 0..=32767 ([`digitizer::LOGICAL_MAX`]).
    0x05, 0x0D, //       Usage Page (Digitizer)
    0x09, 0x02, //       Usage (Pen)
    0xA1, 0x01, //       Collection (Application)
    0x85, digitizer::REPORT_ID, // Report ID (6)
    0x09, 0x20, //         Usage (Stylus)
    0xA1, 0x00, //         Collection (Physical)
    0x09, 0x42, //           Usage (Tip Switch)
    0x09, 0x32, //           Usage (In Range)
    0x15, 0x00, //           Logical Minimum (0)
    0x25, 0x01, //           Logical Maximum (1)
    0x75, 0x01, //           Report Size (1)
    0x95, 0x02, //           Report Count (2)
    0x81, 0x02, //           Input (Data,Var,Abs)
    0x75, 0x06, //           Report Size (6, padding to a byte)
    0x95, 0x01, //           Report Count (1)
    0x81, 0x03, //           Input (Const,Var,Abs)
    0x05, 0x01, //           Usage Page (Generic Desktop)
    0x09, 0x30, //           Usage (X)
    0x09, 0x31, //           Usage (Y)
    0x16, 0x00, 0x00, //     Logical Minimum (0)
    0x26, 0xFF, 0x7F, //     Logical Maximum (32767)
    0x75, 0x10, //           Report Size (16)
    0x95, 0x02, //           Report Count (2)
    0x81, 0x02, //           Input (Data,Var,Abs)
    0xC0, //             End Collection (Physical)
    0xC0, //             End Collection (Application)
];

/// Wire length of the NKRO report (report 1): report ID + modifier byte + the
/// [`keymap::NKRO_BYTES`]-byte bitmap.
const NKRO_REPORT_LEN: usize = 2 + keymap::NKRO_BYTES;
/// Wire length of the consumer report (report 2): report ID + a 16-bit usage.
const CONSUMER_REPORT_LEN: usize = 3;
/// Wire length of the system report (report 3): report ID + an 8-bit usage.
const SYSTEM_REPORT_LEN: usize = 2;
/// Wire length of the mouse report (report 4): report ID + button byte + signed
/// X + signed Y + signed wheel.
const MOUSE_REPORT_LEN: usize = 5;
/// Wire length of the gamepad report (report 5): report ID + a 16-bit button field
/// (little-endian) + four signed axis bytes (X, Y, Z, Rz).
const GAMEPAD_REPORT_LEN: usize = 7;
/// The idle gamepad report: no buttons held, all axes centred. The changed-only
/// cache in [`shared_ep_loop`] starts here and resets to it on (re)configuration and
/// a transport switch, so a still-held gamepad key re-sends rather than being masked.
const GAMEPAD_IDLE: [u8; GAMEPAD_REPORT_LEN] = [REPORT_ID_GAMEPAD, 0, 0, 0, 0, 0, 0];

/// Shared-interface IN endpoint buffer / max-packet size: the largest of the four
/// report wire lengths. The NKRO report is the largest, so this is
/// [`NKRO_REPORT_LEN`]; the assertions keep it the true maximum (and a clean
/// multiple-free size so [`HidWriter::write`] never appends a ZLP for the shorter
/// reports).
const SHARED_WRITE_N: usize = NKRO_REPORT_LEN;
const _: () = assert!(SHARED_WRITE_N >= CONSUMER_REPORT_LEN);
const _: () = assert!(SHARED_WRITE_N >= SYSTEM_REPORT_LEN);
const _: () = assert!(SHARED_WRITE_N >= MOUSE_REPORT_LEN);
const _: () = assert!(SHARED_WRITE_N >= GAMEPAD_REPORT_LEN);
const _: () = assert!(SHARED_WRITE_N >= digitizer::REPORT_LEN);
// The descriptor's NKRO `Report Count` (112) must match the bitmap width.
const _: () = assert!(keymap::NKRO_BYTES * 8 == 0x70);

// ===========================================================================
// USB personality (Normal / MIDI / XInput) + re-enumeration
// ===========================================================================
//
// The normal composite spends all three of the MUSB core's interrupt-IN
// endpoints (boot keyboard, kcp IN, the shared NKRO/consumer/system/mouse/
// gamepad/digitizer interface). USB-MIDI and XInput are NOT HID — they need their
// own bulk/interrupt endpoints — so they cannot be added as report IDs the way the
// mouse, gamepad and digitizer are. Instead the device adopts an *exclusive*,
// re-enumerated personality: it detaches from USB, rebuilds its descriptor set as
// a MIDI (or XInput) device alongside the kcp control interface, and on exit
// returns to the normal composite. The host drives this live over kcp
// ([`crate::kcp::CMD_SYSTEM_SET_USB_MODE`]); the kcp interface is present in every
// mode so the switch is always reversible (and ENTER_DFU / REBOOT always work).

/// The USB device personality the host has selected. `Normal` is the full keeberry
/// composite; `Midi` and `Xinput` are re-enumerated single-purpose devices (each
/// still carrying the kcp control interface). Selected over kcp; a change
/// re-enumerates the device (see [`usb_task`]).
#[derive(Clone, Copy, PartialEq, Eq, defmt::Format)]
pub enum UsbMode {
    /// The full composite: boot keyboard + kcp + shared HID (the power-on default).
    Normal = 0,
    /// A USB-MIDI device (plus kcp): the matrix as a chromatic MIDI controller.
    Midi = 1,
    /// An Xbox 360 (XInput) controller (plus kcp): the matrix as a gamepad.
    Xinput = 2,
}

impl UsbMode {
    /// The [`UsbMode`] for a wire code (`0`/`1`/`2`), or `None` if out of range.
    fn from_code(code: u8) -> Option<Self> {
        match code {
            0 => Some(Self::Normal),
            1 => Some(Self::Midi),
            2 => Some(Self::Xinput),
            _ => None,
        }
    }
}

/// The personality the host has requested — the single source of truth the USB
/// task converges to. [`request_usb_mode`] writes it; [`usb_task`] reads it after
/// each re-enumeration, and [`usb_mode_code`] reports it back over kcp.
static REQUESTED_MODE: AtomicU8 = AtomicU8::new(UsbMode::Normal as u8);

/// Fired by [`request_usb_mode`] when the requested personality changes, to break
/// the running device out of its run loop so [`usb_task`] can re-enumerate. A
/// unit signal (the target is read from [`REQUESTED_MODE`], not carried here).
static MODE_CHANGE: Signal<CriticalSectionRawMutex, ()> = Signal::new();

/// How long to hold the D-/D+ lines at SE0 during a re-enumeration so the host
/// registers the disconnect before the reconnect. Generous (a disconnect is
/// detected in microseconds) so even a slow host tears the old device down fully.
const REENUM_DETACH: Duration = Duration::from_millis(120);

/// How long a re-enumerated personality (MIDI / XInput) has to be configured by the
/// host before [`usb_task`] gives up and reverts to [`UsbMode::Normal`]. Long enough
/// for an unhurried enumeration, short enough that a host-rejected or unsupported
/// personality snaps back to a working keyboard on its own — no replug.
const MODE_CONFIRM: Duration = Duration::from_secs(4);

/// USB D- pad: PA11, alternate function 3 (from `wb32_usb_connect`).
const USB_DM_PIN: gpio::Pin = gpio::Pin::new(gpio::Port::A, 11);
/// USB D+ pad: PA12, alternate function 3.
const USB_DP_PIN: gpio::Pin = gpio::Pin::new(gpio::Port::A, 12);
/// Alternate-function selector that routes PA11/PA12 to the USB PHY (AF3).
const USB_PAD_AF: u8 = 3;

/// Request a USB personality switch (the kcp `SYSTEM.SET_USB_MODE` op). Returns the
/// validated [`UsbMode`] for a known code, or `None` for an out-of-range one. On an
/// actual change it signals [`usb_task`] to re-enumerate; re-requesting the current
/// personality is a no-op, so the host can issue it idempotently.
pub fn request_usb_mode(code: u8) -> Option<UsbMode> {
    let mode = UsbMode::from_code(code)?;
    if REQUESTED_MODE.swap(mode as u8, Ordering::Relaxed) != mode as u8 {
        MODE_CHANGE.signal(());
    }
    Some(mode)
}

/// The currently selected personality as a wire code (the kcp `SYSTEM.GET_USB_MODE`
/// op). This is the target [`usb_task`] converges to; in steady state it is the
/// personality actually enumerated.
pub fn usb_mode_code() -> u8 {
    REQUESTED_MODE.load(Ordering::Relaxed)
}

/// Whether a non-default USB personality (MIDI / XInput) is selected — the device may
/// therefore be mid re-enumeration, holding `CONFIGURED` low with the cable still in
/// for up to [`MODE_CONFIRM`]. The transport supervisor reads it so a personality
/// switch is not mistaken for a USB unplug and flapped onto wireless.
pub(crate) fn usb_personality_active() -> bool {
    REQUESTED_MODE.load(Ordering::Relaxed) != UsbMode::Normal as u8
}

/// Electrically detach from USB by driving D-/D+ (PA11/PA12) low (SE0). The "mini"
/// MUSB core has no soft-connect register — `musb`'s `Bus::disable` is a no-op and
/// the pads *are* the connect (see the module header and [`wb32_usb_init`]) — so
/// forcing SE0 here is the only way to make the host re-enumerate. [`usb_attach`]
/// restores the pads. Re-enumeration only: a normal boot never calls this.
fn usb_detach() {
    gpio::set_output_push_pull(USB_DM_PIN);
    gpio::set_output_push_pull(USB_DP_PIN);
    gpio::set_low(USB_DM_PIN);
    gpio::set_low(USB_DP_PIN);
}

/// Reconnect to USB by restoring D-/D+ (PA11/PA12) to the USB PHY (AF3) and
/// re-asserting MUSB suspend detection, mirroring the connect tail of
/// [`wb32_usb_init`]. The max-drive `CURRENT` setting programmed there for these
/// pads is untouched by the GPIO mode writes, so it persists across the detach.
fn usb_attach() {
    gpio::set_alternate_push_pull(USB_DM_PIN, USB_PAD_AF);
    gpio::set_alternate_push_pull(USB_DP_PIN, USB_PAD_AF);
    UsbInstance::regs()
        .power()
        .modify(|w| w.set_enable_suspend_m(true));
}

/// Drive the USB device, switching personality on the host's command.
///
/// Each iteration builds the device for the currently requested [`UsbMode`] and
/// runs it until the host requests a *different* one (via kcp), then physically
/// re-enumerates — SE0 for [`REENUM_DETACH`], reconnect — and rebuilds for the new
/// personality. The first iteration runs [`UsbMode::Normal`] over the controller
/// [`wb32_usb_init`] already connected at boot, so it does not detach first.
///
/// [`MODE_CHANGE`] is reset before each [`REQUESTED_MODE`] read so a request that
/// arrives during a rebuild is never lost (it re-fires the loop) and a stale signal
/// never tears the fresh device down immediately.
#[embassy_executor::task]
pub async fn usb_task() {
    loop {
        MODE_CHANGE.reset();
        let mode = UsbMode::from_code(REQUESTED_MODE.load(Ordering::Relaxed))
            .unwrap_or(UsbMode::Normal);
        match mode {
            // Normal is the power-on composite and always enumerates, so it runs
            // unguarded. A re-enumerated personality (MIDI / XInput) can be rejected
            // by the host — a descriptor the OS dislikes, or simply an OS with no
            // driver for the class — which would otherwise strand the keyboard off
            // the bus with the control interface gone and no way back. Force
            // CONFIGURED low and race the runner against the watchdog: if the host
            // does not configure the rebuilt device within MODE_CONFIRM, revert to
            // Normal so the board returns to a working state on its own.
            UsbMode::Normal => run_normal().await,
            personality => {
                CONFIGURED.store(false, Ordering::Relaxed);
                if let Either::Second(()) =
                    select(run_personality(personality), mode_confirm_watchdog()).await
                {
                    REQUESTED_MODE.store(UsbMode::Normal as u8, Ordering::Relaxed);
                }
            }
        }
        // A runner returns only on a personality change (or the watchdog revert
        // above). Re-enumerate so the host drops the old device and reads the
        // rebuilt one on the next iteration.
        usb_detach();
        Timer::after(REENUM_DETACH).await;
        usb_attach();
    }
}

/// Run one re-enumerated personality to completion (each returns on a
/// [`MODE_CHANGE`]). Split out so [`usb_task`] can race it against
/// [`mode_confirm_watchdog`] without duplicating the match.
async fn run_personality(mode: UsbMode) {
    match mode {
        UsbMode::Midi => run_midi().await,
        UsbMode::Xinput => run_xinput().await,
        UsbMode::Normal => {}
    }
}

/// Resolve only when the active personality has *failed* to come up: wait up to
/// [`MODE_CONFIRM`] for the host to set [`CONFIGURED`]. If it does, the mode is good
/// — never resolve, leaving the runner to own the race until a real mode change. If
/// it does not, resolve so [`usb_task`] reverts to [`UsbMode::Normal`]. This is the
/// self-heal for a host that *never configures* the device — one that rejects or
/// cannot enumerate the rebuilt descriptor — which would otherwise strand the board
/// off the bus. A host that *does* configure the device but has no class driver for
/// the personality (e.g. macOS for XInput) still sets [`CONFIGURED`], so it does not
/// revert — and that is fine, because the kcp control interface stays live for the
/// host to switch back.
async fn mode_confirm_watchdog() {
    let deadline = Instant::now() + MODE_CONFIRM;
    while Instant::now() < deadline {
        if CONFIGURED.load(Ordering::Relaxed) {
            core::future::pending::<()>().await;
        }
        Timer::after(Duration::from_millis(50)).await;
    }
}

/// Build and run the normal composite until the host selects a different
/// personality. The body matches the original single-mode device — a 6KRO boot
/// keyboard, the kcp raw-HID interface and the shared report-ID interface (NKRO +
/// consumer + system + mouse + gamepad + digitizer) — with [`embassy_usb::UsbDevice::run`]
/// [`join4`]ed against [`keyboard_loop`], [`shared_ep_loop`] and [`kcp_loop`], the
/// whole join raced against [`MODE_CHANGE`] so a mode switch cancels it and returns.
///
/// The descriptor/control buffers, the HID `State`s and the reader/writer handles
/// are locals that live until this future returns (after the race resolves),
/// satisfying the borrows `embassy-usb` requires; returning drops them, freeing the
/// MUSB endpoints for the next personality.
///
/// # Endpoint budget
///
/// The MUSB core on this part exposes EP0 plus EP1–EP3 (`wb32fq95` profile, four
/// endpoints, each bidirectional). `embassy-usb` assigns addresses while building
/// the interfaces in order, picking the lowest free endpoint per direction:
/// the boot keyboard takes EP1 IN, the raw-HID interface takes EP1 OUT + EP2 IN,
/// and the shared interface then takes EP3 IN — the keyboard keeps the endpoint it
/// already enumerated with. NKRO, consumer, system, the mouse, the gamepad and the
/// digitizer all ride EP3 IN *together* via report IDs, so none needs a new
/// endpoint: still three IN endpoints and one OUT within EP1–EP3.
async fn run_normal() {
    let driver = Driver::new();

    let mut config = Config::new(VID, PID);
    config.manufacturer = Some(MANUFACTURER);
    // The wired product string is the fixed USB-link name ("Akko 5075B USB"), not
    // the active output transport. It is deliberately *not* transport-reactive:
    // each host sees the name of *its own* link to the board, and the cable host's
    // link is always USB — the 2.4G and BT hosts get their own mode names via the
    // product / DEVINFO strings [`crate::wireless`] pushes to the radio. Mirroring
    // the live transport here would mean re-enumerating USB (driving D-/D+ to SE0,
    // see [`usb_detach`]) on every Fn-driven mode switch, tearing down the live kcp
    // control link for a cosmetic label — the exact fragility the re-enumeration
    // path avoids by firing only on the rare personality (`UsbMode`) change. So the
    // wired name is the single-source-of-truth [`wireless::Devs::Usb`] name, fixed.
    config.product = Some(wireless::Devs::Usb.device_name());
    config.serial_number = Some("0001");
    config.max_power = 100;

    // Backing storage for the device. Sized comfortably for the three HID
    // interfaces (keyboard + shared + raw-HID): their combined descriptors are
    // well under 256 bytes.
    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut msos_descriptor = [0u8; 64];
    let mut control_buf = [0u8; 64];
    let mut hid_state = State::new();
    let mut kcp_state = State::new();
    let mut shared_state = State::new();
    // Declared before the builder so it outlives the device that borrows it.
    let mut state_handler = UsbStateHandler;

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    // Interface 0: 6KRO boot keyboard (8-byte input report). Allocating the writer
    // places the interface and its IN endpoint in the enumerated descriptor set;
    // [`keyboard_loop`] drives it. Built first so it keeps the IN endpoint it
    // already enumerates with. This is the keyboard the host always reads for the
    // first six keys (and a BIOS would, once boot protocol lands); under NKRO the
    // overflow rides the shared interface (interface 2) instead — the two never
    // share a usage, so a host that merges both interfaces never double-types.
    //
    // This advertises HID subclass 0 / protocol 0: a report-protocol keyboard
    // using the standard `KeyboardReport` shape, not a BIOS boot-protocol
    // interface. Full boot-protocol support (subclass 1 / protocol 1 plus
    // SET_PROTOCOL handling) is a deliberate non-goal: the report already *is* the
    // fixed 8-byte boot shape, so mainstream UEFI hosts drive it in report protocol.
    let hid_config = hid::Config {
        report_descriptor: KeyboardReport::desc(),
        request_handler: None,
        poll_ms: 10,
        max_packet_size: 8,
    };
    let mut keyboard: HidWriter<'_, Driver<'_>, 8> =
        HidWriter::new(&mut builder, &mut hid_state, hid_config);

    // Interface 1: raw-HID vendor interface carrying kcp. 32-byte OUT (host ->
    // device requests) and 32-byte IN (device -> host replies), no report ID,
    // polled every 1 ms for low config latency. `HidReaderWriter::new` allocates
    // both an interrupt IN and an interrupt OUT endpoint; [`kcp_loop`] drives the
    // split halves.
    let kcp_config = hid::Config {
        report_descriptor: KCP_HID_DESCRIPTOR,
        request_handler: None,
        poll_ms: 1,
        max_packet_size: KCP_REPORT_LEN as u16,
    };
    let kcp_hid: HidReaderWriter<'_, Driver<'_>, KCP_REPORT_LEN, KCP_REPORT_LEN> =
        HidReaderWriter::new(&mut builder, &mut kcp_state, kcp_config);
    let (mut kcp_reader, mut kcp_writer) = kcp_hid.split();

    // Interface 2: shared report-ID interface (NKRO keyboard + consumer + system
    // control + mouse + gamepad + digitizer). A single IN endpoint carrying all
    // six report IDs, polled every 1 ms for low rollover/pointer latency;
    // [`shared_ep_loop`] drives it from the NKRO overflow, consumer usage, held
    // mouse / gamepad keys and the host-set digitizer position. Built last, so it
    // takes the remaining IN endpoint (EP3 IN) and leaves the keyboard/raw-HID
    // endpoints where they enumerated. One [`HidWriter`] ⇒ one IN endpoint, so the
    // digitizer (like the mouse, gamepad and NKRO) costs no extra endpoint.
    let shared_config = hid::Config {
        report_descriptor: SHARED_HID_DESCRIPTOR,
        request_handler: None,
        poll_ms: 1,
        max_packet_size: SHARED_WRITE_N as u16,
    };
    let mut shared: HidWriter<'_, Driver<'_>, SHARED_WRITE_N> =
        HidWriter::new(&mut builder, &mut shared_state, shared_config);

    // Track configuration state so the report loop only writes when the host can
    // receive (see [`keyboard_loop`]).
    builder.handler(&mut state_handler);

    let mut usb = builder.build();

    // Run the device and the three report loops together, raced against the
    // personality-change signal: `usb.run()` services the bus while `keyboard_loop`
    // reports key state, `shared_ep_loop` sends the shared reports and `kcp_loop`
    // answers config requests. None of the four ever returns on its own; the race's
    // other arm ([`MODE_CHANGE`]) is what ends this and triggers re-enumeration. The
    // shared-interface write lives in its own future so its blocking await cannot
    // stall the boot keyboard path (see [`shared_ep_loop`]).
    select(
        join4(
            usb.run(),
            keyboard_loop(&mut keyboard),
            shared_ep_loop(&mut shared),
            kcp_loop(&mut kcp_reader, &mut kcp_writer),
        ),
        MODE_CHANGE.wait(),
    )
    .await;
}

/// Build and run the USB-MIDI personality until the host selects a different one.
///
/// The device is a [`MidiClass`] (one bulk IN + one bulk OUT) plus the kcp raw-HID
/// control interface — kcp is kept so the host can switch back and so ENTER_DFU /
/// REBOOT still work. [`crate::midi::run`] maps the matrix to notes in place of the
/// keyboard/shared loops (which have no interface in this mode); [`kcp_loop`] still
/// serves config. The whole join is raced against [`MODE_CHANGE`].
async fn run_midi() {
    let driver = Driver::new();

    let mut config = Config::new(VID, PID);
    config.manufacturer = Some(MANUFACTURER);
    // The MIDI personality is not one of the connection modes, so its name does not
    // come from [`wireless::Devs::device_name`]; it shares the same "Akko 5075B"
    // brand so the board keeps one identity when it re-enumerates as a MIDI device.
    config.product = Some("Akko 5075B MIDI");
    config.serial_number = Some("0001");
    config.max_power = 100;

    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut msos_descriptor = [0u8; 64];
    let mut control_buf = [0u8; 64];
    let mut kcp_state = State::new();
    let mut state_handler = UsbStateHandler;

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    // kcp control interface (kept in every personality for reversibility / DFU).
    let kcp_config = hid::Config {
        report_descriptor: KCP_HID_DESCRIPTOR,
        request_handler: None,
        poll_ms: 1,
        max_packet_size: KCP_REPORT_LEN as u16,
    };
    let kcp_hid: HidReaderWriter<'_, Driver<'_>, KCP_REPORT_LEN, KCP_REPORT_LEN> =
        HidReaderWriter::new(&mut builder, &mut kcp_state, kcp_config);
    let (mut kcp_reader, mut kcp_writer) = kcp_hid.split();

    // One embedded MIDI in/out jack pair, 64-byte bulk packets (full-speed max).
    let mut midi = MidiClass::new(&mut builder, 1, 1, 64);

    builder.handler(&mut state_handler);
    let mut usb = builder.build();

    select(
        join3(
            usb.run(),
            kcp_loop(&mut kcp_reader, &mut kcp_writer),
            midi::run(&mut midi),
        ),
        MODE_CHANGE.wait(),
    )
    .await;
}

/// Build and run the XInput personality until the host selects a different one.
///
/// The device is an Xbox 360 controller ([`crate::xinput`]) plus the kcp control
/// interface. XInput is built *first* so its auto-allocated interrupt IN/OUT take
/// `0x81`/`0x01` (the addresses its magic descriptor names); kcp then takes the
/// remaining endpoints. [`crate::xinput::run`] maps the matrix to gamepad buttons;
/// [`kcp_loop`] serves config (so the host can switch back). Raced against
/// [`MODE_CHANGE`].
async fn run_xinput() {
    let driver = Driver::new();

    let mut config = Config::new(xinput::VID, xinput::PID);
    config.manufacturer = Some(MANUFACTURER);
    // XInput keeps the Xbox 360 controller's own "Controller" product string (paired
    // with the 045E:028E VID/PID above): the host binds it as an Xbox pad by that
    // identity, so it is deliberately not rebranded to an "Akko 5075B" name — only
    // the manufacturer is unified with the rest of the board.
    config.product = Some("Controller");
    config.serial_number = Some("0001");
    config.max_power = 100;
    // The Xbox 360 controller advertises a vendor device class so the XUSB driver
    // binds; the keeberry identity returns when the personality is left.
    config.device_class = 0xFF;
    config.device_sub_class = 0xFF;
    config.device_protocol = 0xFF;
    // XInput is a vendor-class device (0xFF), not a 0xEF IAD composite; the XUSB/xpad
    // drivers bind the vendor interface by its FF/5D/01 class triple and the host
    // handles the kcp HID interface independently. `Config::new` defaults
    // `composite_with_iads` true, which makes `Builder::new` assert the device triple
    // is 0xEF/0x02/0x01 and panic on this 0xFF class — so it must be cleared here.
    config.composite_with_iads = false;
    // The real wired Xbox 360 controller uses an 8-byte control endpoint
    // (bMaxPacketSize0 = 8); match it so EP0 looks identical to the device the XUSB
    // driver expects. embassy defaults this to 64.
    config.max_packet_size_0 = 8;

    let mut config_descriptor = [0u8; 256];
    let mut bos_descriptor = [0u8; 256];
    let mut msos_descriptor = [0u8; 64];
    let mut control_buf = [0u8; 64];
    let mut kcp_state = State::new();
    let mut state_handler = UsbStateHandler;

    let mut builder = Builder::new(
        driver,
        config,
        &mut config_descriptor,
        &mut bos_descriptor,
        &mut msos_descriptor,
        &mut control_buf,
    );

    // XInput is built first so its auto-allocated interrupt IN/OUT take the first
    // slots (0x81 IN / 0x01 OUT), the addresses its magic descriptor names. Both are
    // driven by `xinput::run`: IN sends the gamepad reports, OUT is drained of the
    // host's rumble/LED reports so they cannot back up the pipe.
    let (mut xinput_in, mut xinput_out) = xinput::build(&mut builder);

    // kcp control interface takes the remaining endpoints.
    let kcp_config = hid::Config {
        report_descriptor: KCP_HID_DESCRIPTOR,
        request_handler: None,
        poll_ms: 1,
        max_packet_size: KCP_REPORT_LEN as u16,
    };
    let kcp_hid: HidReaderWriter<'_, Driver<'_>, KCP_REPORT_LEN, KCP_REPORT_LEN> =
        HidReaderWriter::new(&mut builder, &mut kcp_state, kcp_config);
    let (mut kcp_reader, mut kcp_writer) = kcp_hid.split();

    builder.handler(&mut state_handler);
    let mut usb = builder.build();

    select(
        join3(
            usb.run(),
            kcp_loop(&mut kcp_reader, &mut kcp_writer),
            xinput::run(&mut xinput_in, &mut xinput_out),
        ),
        MODE_CHANGE.wait(),
    )
    .await;
}

/// Split a [`keymap::Report`] into the disjoint dual-send halves used when NKRO is
/// enabled: the 6KRO boot report (EP1 / the wireless boot frame) and the overflow
/// bitmap (EP3 report 1 / `md_send_nkro`).
///
/// The split is built so that **every held usage lands on exactly one interface —
/// none dropped, none duplicated** — which is what keeps the dual-send free of
/// double-typing once a host merges the two interfaces:
///
/// 1. The out-of-range usages [`keymap::Report::high`] (`> 0x6F`, which the bitmap
///    cannot represent) go into the boot report first — it is the only interface
///    that can carry them. They are disjoint from the bitmap by construction.
/// 2. Any remaining boot slots are filled from the lowest bitmap usages, each
///    *cleared* from the returned overflow copy so it cannot also appear there.
/// 3. Whatever bitmap usages are left stay in the overflow.
///
/// So `boot6` = (high usages) ∪ (some low usages) and `overflow` = (the rest of the
/// low usages), with an empty intersection. The modifier byte is the shared
/// `report.boot.modifier`. A seventh-plus out-of-range usage cannot be conveyed on
/// either interface (the boot report is full and the bitmap cannot hold it) and is
/// dropped — the same loss 6KRO signals past six keys, and vanishingly rare.
fn split_nkro(report: &keymap::Report) -> (KeyboardReport, [u8; keymap::NKRO_BYTES]) {
    let mut overflow = report.nkro_bits;
    let mut boot6 = KeyboardReport::default();
    boot6.modifier = report.boot.modifier;
    let mut placed = 0usize;
    // 1. Out-of-range usages must ride the boot report (no bitmap representation).
    for &usage in report.high.iter() {
        if usage == 0 {
            continue; // zero-padding past the real high usages
        }
        if placed >= boot6.keycodes.len() {
            break; // boot report full; an excess high usage is dropped (see docs)
        }
        boot6.keycodes[placed] = usage;
        placed += 1;
    }
    // 2. Fill any remaining boot slots from the lowest bitmap usages, clearing each
    //    from the overflow so the two halves stay disjoint.
    'outer: for (byte_idx, byte) in overflow.iter_mut().enumerate() {
        while *byte != 0 {
            if placed >= boot6.keycodes.len() {
                break 'outer;
            }
            let bit = byte.trailing_zeros();
            *byte &= *byte - 1; // clear lowest set bit
            boot6.keycodes[placed] = (byte_idx * 8 + bit as usize) as u8;
            placed += 1;
        }
    }
    (boot6, overflow)
}

/// Scan the matrix, run the [`crate::keymap`] engine and report key state over
/// the boot keyboard HID IN endpoint (EP1).
///
/// Owns a [`matrix::Debouncer`] and the [`keymap::LayerState`], polling once a
/// millisecond. Each scan builds the [`keymap::Report`] ([`keymap::compute_report`])
/// — the 6KRO boot report plus the full NKRO bitmap — and, from the same debounced
/// state and active-layer mask, resolves the consumer usage
/// ([`keymap::consumer_usage`]) which it publishes to [`CONSUMER_USAGE`] for
/// [`shared_ep_loop`] to send. It writes the 6KRO boot report itself (EP1) and, when
/// NKRO is enabled, publishes the overflow bitmap to [`NKRO_OUT`] for
/// [`shared_ep_loop`] to send on the shared EP3 report 1 — keeping that endpoint's
/// (potentially blocking) write off this key path, while the first six keys always
/// reach the host via EP1 here. Sending the boot report is gated on [`CONFIGURED`]:
/// while configured it is written only when it differs from the last one the host
/// received, so a steady state (including all keys up) does not spam the endpoint;
/// the device stays responsive because [`embassy_usb::UsbDevice::run`] is joined
/// alongside. This also subsumes the old standalone RTT scan task, emitting key
/// activity at `debug` level whenever the report changes.
///
/// # Rollover modes (no double-typing)
///
/// With NKRO disabled (the power-on default) EP1 carries the standard 6KRO report
/// and the EP3 NKRO report is idle — the plain 6KRO boot path. With
/// NKRO enabled the held set is split disjointly by [`split_nkro`]: the lowest six
/// usages ride the EP1 boot report and the rest the EP3 overflow bitmap. Because no
/// usage is ever in both, a host that merges the two interfaces (every modern OS
/// does) sees each key exactly once — the dual-send cannot double-type. This is the
/// vendor's wireless `wireless_send_nkro` pattern (boot report + overflow bitmap)
/// applied to USB.
///
/// The configured-gate is load-bearing for correctness, not just efficiency.
/// The `musb` fork's `EndpointIn::write` waits only on `TXCSRL.TxPktRdy` (the
/// FIFO being free) and returns `Ok` even when the endpoint is not enabled — it
/// never `wait_enabled()`s. A report written before the host configures the
/// interface, or during a bus reset, would therefore be cached as "sent" yet
/// never reach the host; the next equal report would then be suppressed and the
/// keypress lost. Gating on `CONFIGURED` and resyncing on every (re)configuration
/// closes that hole — it is the firmware-side contract for this `musb` write
/// semantics, and what makes the changed-only optimisation correct.
async fn keyboard_loop<'a>(keyboard: &mut HidWriter<'a, Driver<'a>, 8>) {
    let mut debouncer = matrix::Debouncer::new();
    let mut layer = keymap::LayerState::new();
    // Cache of the last report the host has actually received. Just after a
    // `SET_CONFIGURATION` the host assumes all keys are up, so the empty report
    // is the correct starting point; only deltas are sent thereafter.
    let mut last = KeyboardReport::default();
    let mut was_configured = false;
    // The wireless path's equivalent change-detection cache (the radio reports
    // only on change, like QMK's host-side dedup), kept separate from the USB
    // `last` so switching transports cannot mask a held key on either side.
    let mut last_wls = KeyboardReport::default();
    // The wireless NKRO-overflow cache, parallel to `last_wls`, so a change in the
    // overflow bitmap alone (the boot half unchanged) still triggers a radio
    // dual-send. Empty when NKRO is disabled, so it never forces a send then.
    let mut last_wls_overflow = [0u8; keymap::NKRO_BYTES];
    // Last observed transport, to detect a switch and resync both caches.
    let mut prev_transport = wireless::transport();
    // Previous debounced scan and active-layer mask: the edge basis and the
    // keycode-resolution layer mask the matrix fold reads each scan. Kept current
    // at the tail of every loop iteration below.
    let mut prev_matrix = [0u16; matrix::NUM_ROWS];
    let mut prev_active = layer.active();

    loop {
        // Always scan and debounce, even while unconfigured, so key state is
        // current the moment the host configures the interface. Time the input
        // pipeline (scan + debounce + keymap) for the latency telemetry: this is
        // the real per-iteration firmware processing cost, excluding the 1 ms
        // inter-scan delay below and the USB transfer.
        let t0 = Instant::now();
        let debounced = debouncer.update(matrix::scan());
        // The matrix fold runs between debounce and the report build: the timed
        // engine returns the effective matrix (physical scan with combo-claimed,
        // auto-shift and leader positions suppressed) that `compute_report`
        // resolves. It gates per feature on the ANY flag, so `effective ==
        // debounced` and the report is byte-identical to today when nothing is set
        // up. `t0` is this scan's timestamp; `prev_matrix`/`prev_active` (maintained
        // every scan below) supply the edge basis and the keycode-resolution mask.
        let mut effective = debounced;
        let matrix_ctx = features::Ctx {
            now: t0,
            active_layers: prev_active,
            prev_matrix: &prev_matrix,
        };
        features::run_on_matrix(&matrix_ctx, &mut effective);
        let mut report = keymap::compute_report(effective, &mut layer, t0);
        // The overlay fold merges any resolved timed-behaviour output — a resolved
        // tap-dance tap/hold, a fired combo's action, an in-flight macro frame —
        // into the report before it is routed to either transport. Gated per
        // feature, so the report is unchanged while the engine is unused.
        let overlay_ctx = features::Ctx {
            now: t0,
            active_layers: layer.active(),
            prev_matrix: &prev_matrix,
        };
        features::run_on_overlay(&overlay_ctx, &mut report);
        // Per-scan tick for timeout-driven feature state — e.g. the unicode player
        // advancing its injected codepoint sequence; gated, so idle features cost one load.
        features::run_on_tick(t0);
        telemetry::record_proc(t0.elapsed());
        telemetry::inc_scan();
        telemetry::set_active_layers(layer.active());

        // Refresh the edge/layer basis the matrix fold reads on the next scan.
        let active = layer.active();
        prev_matrix = debounced;
        prev_active = active;

        // Publish the consumer usage resolved from this same scan (under the same
        // active layers as the keyboard report) for `shared_ep_loop` to send. The
        // consumer HID write is done there, not here, so a host that is slow to
        // poll the shared endpoint can never stall this key-scanning path. Resolved
        // from `effective`, not the raw `debounced` scan, so combo suppression
        // applies to a consumer-key member too — otherwise a combo member that
        // resolves to a consumer usage would leak on this path while suppressed on
        // the keyboard one. (`effective == debounced` until a combo is configured.)
        CONSUMER_USAGE.store(
            keymap::consumer_usage(effective, layer.active()),
            Ordering::Relaxed,
        );

        // Publish the held mouse keys resolved from this same scan for
        // `shared_ep_loop` to accelerate and send (report 4). Like the consumer
        // usage this is decoupled from the EP3 write, and resolved from `effective`
        // so combo suppression applies to a mouse-key combo member too.
        MOUSE_KEYS.store(
            keymap::mouse_keys(effective, layer.active()),
            Ordering::Relaxed,
        );

        // Publish the held gamepad keys resolved from this same scan for
        // `shared_ep_loop` to decode and send (report 5). Decoupled from the EP3
        // write like the mouse keys, and resolved from `effective` so combo
        // suppression applies to a gamepad-key combo member too.
        GAMEPAD_KEYS.store(
            keymap::gamepad_keys(effective, layer.active()),
            Ordering::Relaxed,
        );

        // Resolve the rollover mode for this scan and derive the EP1 boot report
        // and the NKRO overflow from the single computed report:
        //   * NKRO off — `ep1` is the standard 6KRO boot report (first six keys /
        //     ErrorRollOver) and the overflow is empty: a pure 6KRO boot path, and
        //     nothing rides the EP3 NKRO report.
        //   * NKRO on  — `split_nkro` partitions the held set so `ep1` carries the
        //     lowest six usages and `overflow` the rest, disjointly (no
        //     double-typing; see the loop docs and `split_nkro`).
        let nkro_on = nkro_enabled();
        let (ep1, overflow_mods, overflow) = if nkro_on {
            let (boot6, ovf) = split_nkro(&report);
            (boot6, report.boot.modifier, ovf)
        } else {
            (report.boot, 0u8, [0u8; keymap::NKRO_BYTES])
        };
        // Hand the NKRO overflow to `shared_ep_loop` for the EP3 report-1 write
        // (USB). When NKRO is off this publishes the idle value, so the shared loop
        // releases the NKRO report once and then stays quiescent (it dedups).
        publish_nkro(overflow_mods, overflow);

        // Transport-aware routing. The USB block keeps the wired-only firmware's
        // exact send policy (configured-gate, rising-edge resync, changed-only
        // check); a wireless transport routes the report to the radio builders
        // instead. `configured` and the `was_configured` edge are tracked every
        // loop, independent of transport (see below).
        let transport = wireless::transport();
        let configured = CONFIGURED.load(Ordering::Relaxed);

        // A transport switch invalidates the "last sent" caches: the link we move
        // to has its own idea of host state, and the one we left may have reset
        // while we were away. Resyncing here is what lets a USB reconfigure that
        // happened *during* a wireless excursion still re-send a held report on
        // return — the USB rising edge alone would have been missed.
        if transport != prev_transport {
            last = KeyboardReport::default();
            last_wls = KeyboardReport::default();
            last_wls_overflow = [0u8; keymap::NKRO_BYTES];
        }
        prev_transport = transport;

        if transport == wireless::Devs::Usb {
            if configured {
                // Rising edge: the interface was just (re)configured and the host's
                // key state is empty again. Reset the cache so a key held at this
                // instant — including one held across a USB reset — is resent rather
                // than being masked by the changed-only check below.
                if !was_configured {
                    last = KeyboardReport::default();
                }

                // EP1 carries the 6KRO boot report (`report.boot` when NKRO is off,
                // the lowest-six `boot6` when on). The NKRO overflow goes out on EP3
                // from `shared_ep_loop` (published above), off this path so its
                // blocking write can never stall matrix scanning or the boot report.
                if ep1.modifier != last.modifier || ep1.keycodes != last.keycodes {
                    // Log that a report changed, but not its contents — the raw
                    // held keycodes/modifiers are keystrokes and must not land in
                    // the debug log.
                    defmt::debug!("hid report changed: nkro={=bool}", nkro_on);
                    if keyboard.write_serialize(&ep1).await.is_ok() {
                        last = ep1;
                        telemetry::inc_report();
                    }
                }
            }
        } else {
            // Wireless: emit only on change (the radio is edge-driven). The 8-byte
            // boot report is `[modifier, reserved, key0..key5]`, matching the vendor
            // `report_keyboard_t`. When NKRO is on, the dual-send adds the overflow
            // bitmap via `send_nkro` (the vendor `wireless_send_nkro` split), so a
            // change in the overflow alone still re-sends. Advance the caches only
            // when the send actually went out (link up *and* the TX queue accepted
            // it); a dropped report is retried next scan instead of being masked as
            // already-sent, so a held key survives a reconnect or a momentary
            // queue-full. The overflow is empty when NKRO is off, so its comparison
            // never forces a send then (the boot path stays byte-identical).
            let changed = ep1.modifier != last_wls.modifier
                || ep1.keycodes != last_wls.keycodes
                || (nkro_on && overflow != last_wls_overflow);
            if changed {
                let kb = [
                    ep1.modifier,
                    ep1.reserved,
                    ep1.keycodes[0],
                    ep1.keycodes[1],
                    ep1.keycodes[2],
                    ep1.keycodes[3],
                    ep1.keycodes[4],
                    ep1.keycodes[5],
                ];
                let sent = if nkro_on {
                    wireless::send_nkro(&kb, &overflow)
                } else {
                    wireless::send_keyboard(&kb)
                };
                if sent {
                    last_wls = ep1;
                    last_wls_overflow = overflow;
                    telemetry::inc_report();
                }
            }
        }

        // Track the CONFIGURED edge every loop, regardless of transport, so the
        // USB rising-edge resync still fires after a reconfigure that landed
        // while a wireless transport was active.
        was_configured = configured;
        Timer::after(Duration::from_millis(1)).await;
    }
}

/// Whether a consumer usage is actually a System Control usage (`0x81..=0x83`,
/// power / sleep / wake), which rides report 3 rather than the consumer report 2.
/// Mirrors the split the wireless `wireless::send_consumer` makes, so the two
/// transports route system keys identically.
fn is_system_usage(usage: u16) -> bool {
    matches!(usage, 0x81..=0x83)
}

/// Write the consumer (report 2) / system (report 3) change on the shared
/// interface, releasing the channel being left so a held usage can never stick on
/// a channel after switching. Returns whether every needed write went out.
///
/// Only one usage is active at a time (the keymap resolves one consumer usage per
/// scan), so a transition touches at most the two channels involved: when the new
/// usage is on a different channel than the last, the old channel is first released
/// (an empty report), then the new usage is asserted on its channel (a zero usage
/// is the idle/release on that channel). For the default keymap — which binds no
/// system usage — `is_system_usage` is always false, so this is exactly one
/// consumer write per change — the 6KRO consumer path — with a report-ID prefix.
async fn write_extra<'a>(
    shared: &mut HidWriter<'a, Driver<'a>, SHARED_WRITE_N>,
    usage: u16,
    last: u16,
) -> bool {
    let new_sys = is_system_usage(usage);
    let old_sys = is_system_usage(last);
    let mut ok = true;
    // Leaving a channel for the other one (or for idle): release it first.
    if last != 0 && old_sys != new_sys && old_sys {
        ok &= shared.write(&[REPORT_ID_SYSTEM, 0]).await.is_ok();
    }
    if new_sys {
        // Switching consumer -> system: clear the consumer report we were holding.
        if last != 0 && !old_sys {
            ok &= shared.write(&[REPORT_ID_CONSUMER, 0, 0]).await.is_ok();
        }
        ok &= shared.write(&[REPORT_ID_SYSTEM, usage as u8]).await.is_ok();
    } else {
        let [lo, hi] = usage.to_le_bytes();
        ok &= shared.write(&[REPORT_ID_CONSUMER, lo, hi]).await.is_ok();
    }
    ok
}

/// Drive the shared report-ID interface (EP3): the NKRO keyboard (report 1),
/// consumer control (report 2), system control (report 3), the mouse (report 4) and
/// the gamepad (report 5), decoupled from the matrix scan and the EP1 boot report.
///
/// [`keyboard_loop`] publishes the held consumer usage to [`CONSUMER_USAGE`], the
/// NKRO overflow to [`NKRO_OUT`], the held mouse keys to [`MOUSE_KEYS`] and the held
/// gamepad keys to [`GAMEPAD_KEYS`] each scan; this loop reads them and writes the
/// host. The five report types share EP3's single IN endpoint, so their writes
/// serialise here — and, deliberately, off the keyboard path: the `musb` fork's IN
/// `write` waits on `TXCSRL.TxPktRdy` with no timeout, so a host slow to poll EP3
/// would block this loop indefinitely; keeping that `await` out of [`keyboard_loop`]
/// means it can never stall matrix scanning or the EP1 boot report (the first six
/// keys keep flowing even if EP3 is wedged). Wireless NKRO is dual-sent by
/// [`keyboard_loop`] over the radio, so this loop's NKRO write is USB-only; the
/// consumer/system wireless send (`send_consumer`) and the mouse send
/// (the vendor `md_send_mouse`, 0xA8) both run on the wireless transport too. The
/// gamepad and digitizer reports are USB-only — the vendor radio protocol carries no
/// frame for either — so they are sent only on the USB transport.
///
/// Mirrors the keyboard's send policy on each sub-report, polling at 1 ms: gated on
/// [`CONFIGURED`], written only on change from the last value the host received, and
/// resynced on the configuration rising edge and on a transport switch — so a key
/// held across a (re)configuration, bus reset or transport change is re-sent rather
/// than masked by the changed-only check. Like the keyboard report (see
/// [`keyboard_loop`]), the `CONFIGURED` gate is what keeps a report from being
/// cached as "sent" while the endpoint is not drainable.
async fn shared_ep_loop<'a>(shared: &mut HidWriter<'a, Driver<'a>, SHARED_WRITE_N>) {
    // Consumer/system change-detection (USB). 0 (idle) is the correct start.
    let mut last_usage: u16 = 0;
    // NKRO report change-detection (USB): the last modifier + overflow bitmap sent.
    let mut last_nkro_mods: u8 = 0;
    let mut last_nkro_bits = [0u8; keymap::NKRO_BYTES];
    let mut was_configured = false;
    // The wireless consumer/system change-detection cache (see [`keyboard_loop`]).
    let mut last_wls_usage: u16 = 0;
    // Mouse accelerator (movement timing) and the last button byte sent. Buttons
    // are absolute HID state, deduped like `last_usage` (advanced only on a
    // successful write); movement/wheel are relative deltas the accelerator emits.
    let mut mouse = mouse::Accel::new();
    let mut last_mouse_buttons: u8 = 0;
    // Timestamp of the previous iteration, for the real elapsed time the mouse
    // accelerator integrates (a slow host can stretch this loop past 1 ms).
    let mut mouse_last = Instant::now();
    // The last full gamepad report (report 5) the host received. The whole report is
    // absolute state (buttons + axes), so it is deduped as a unit and advances only
    // on a successful write — a dropped change is retried next tick.
    let mut last_gamepad = GAMEPAD_IDLE;
    // The last digitizer report (report 6) the host received. Host-set absolute
    // state, deduped as a unit like the gamepad and advancing only on a successful
    // write. USB-only: there is no vendor radio frame for the digitizer.
    let mut last_digitizer = digitizer::IDLE;
    // Last observed transport, to detect a switch and resync the caches.
    let mut prev_transport = wireless::transport();

    loop {
        let transport = wireless::transport();
        let configured = CONFIGURED.load(Ordering::Relaxed);
        let now = Instant::now();
        let dt = now.saturating_duration_since(mouse_last);
        mouse_last = now;

        // A transport switch resyncs the "last sent" caches (see [`keyboard_loop`]).
        if transport != prev_transport {
            last_usage = 0;
            last_nkro_mods = 0;
            last_nkro_bits = [0u8; keymap::NKRO_BYTES];
            last_wls_usage = 0;
            mouse.reset();
            last_mouse_buttons = 0;
            last_gamepad = GAMEPAD_IDLE;
            last_digitizer = digitizer::IDLE;
        }
        prev_transport = transport;

        if transport == wireless::Devs::Usb {
            if configured {
                // Rising edge: the host assumes every shared report is idle again
                // after (re)configuration, so forget what we sent and let a still-held
                // report re-send below rather than be masked by the changed-only check.
                if !was_configured {
                    last_usage = 0;
                    last_nkro_mods = 0;
                    last_nkro_bits = [0u8; keymap::NKRO_BYTES];
                    mouse.reset();
                    last_mouse_buttons = 0;
                    last_gamepad = GAMEPAD_IDLE;
                    last_digitizer = digitizer::IDLE;
                }

                // --- NKRO keyboard (report 1) ---
                // The overflow keys (those beyond the six in the EP1 boot report).
                // Empty when NKRO is disabled, so after one release write this stays
                // quiescent and the EP3 NKRO report carries nothing.
                let (nkro_mods, nkro_bits) = load_nkro();
                if nkro_mods != last_nkro_mods || nkro_bits != last_nkro_bits {
                    let mut buf = [0u8; NKRO_REPORT_LEN];
                    buf[0] = REPORT_ID_NKRO;
                    buf[1] = nkro_mods;
                    buf[2..].copy_from_slice(&nkro_bits);
                    if shared.write(&buf).await.is_ok() {
                        last_nkro_mods = nkro_mods;
                        last_nkro_bits = nkro_bits;
                    }
                }

                // --- Consumer (report 2) / system (report 3) ---
                let usage = CONSUMER_USAGE.load(Ordering::Relaxed);
                if usage != last_usage {
                    defmt::debug!("shared extra: usage={=u16:#06x}", usage);
                    if write_extra(shared, usage, last_usage).await {
                        last_usage = usage;
                    }
                }

                // --- Mouse (report 4) ---
                // The accelerator turns the held mouse keys into this tick's signed
                // X/Y/wheel deltas (`0` between movement ticks). Movement/wheel are
                // relative, so a non-zero delta is always sent; buttons are absolute,
                // so a report also goes out whenever the button byte changed — and the
                // button cache advances only on a successful write, so a dropped
                // button change is retried next tick (a dropped movement delta is just
                // skipped, negligible). When idle (no delta, buttons unchanged)
                // nothing is sent and the cursor holds.
                let mkeys = MOUSE_KEYS.load(Ordering::Relaxed);
                let (mx, my, mwheel) = mouse.step(mkeys, dt);
                let mbtns = mouse::buttons(mkeys);
                if mx != 0 || my != 0 || mwheel != 0 || mbtns != last_mouse_buttons {
                    let buf = [REPORT_ID_MOUSE, mbtns, mx as u8, my as u8, mwheel as u8];
                    if shared.write(&buf).await.is_ok() {
                        last_mouse_buttons = mbtns;
                    }
                }

                // --- Gamepad (report 5) ---
                // The held gamepad keys decode directly into the report: the 16-bit
                // button field (little-endian) and the four signed axis bytes, each
                // pinned to full deflection while a direction key is held and centred
                // otherwise. The whole report is absolute, so it is sent whenever it
                // differs from the last one the host received (a release re-centres
                // and must be reported); the cache advances only on a successful
                // write. Idle and unchanged ⇒ nothing is sent.
                let gkeys = GAMEPAD_KEYS.load(Ordering::Relaxed);
                let gbtns = gamepad::buttons(gkeys);
                let [gx, gy, gz, grz] = gamepad::axes(gkeys);
                let gbuf = [
                    REPORT_ID_GAMEPAD,
                    gbtns as u8,
                    (gbtns >> 8) as u8,
                    gx as u8,
                    gy as u8,
                    gz as u8,
                    grz as u8,
                ];
                if gbuf != last_gamepad && shared.write(&gbuf).await.is_ok() {
                    last_gamepad = gbuf;
                }

                // --- Digitizer (report 6) ---
                // The host-set absolute contact (position + tip/in-range), packed by
                // [`digitizer::report`]. Absolute state like the gamepad: sent whenever
                // it differs from the last one the host received, the cache advancing
                // only on a successful write. Idle and unchanged ⇒ nothing is sent, so
                // a board the host never drives reports no digitizer traffic.
                let dbuf = digitizer::report();
                if dbuf != last_digitizer && shared.write(&dbuf).await.is_ok() {
                    last_digitizer = dbuf;
                }
            }
        } else {
            // Wireless: consumer / system over the radio. `send_consumer` splits
            // System Control usages (0x81..=0x83) from consumer-page usages, per the
            // vendor `wireless_send_extra`. NKRO over
            // the radio is dual-sent by `keyboard_loop`, so nothing to do for it
            // here. Advance the cache only when the usage was actually enqueued, so a
            // drop (link down or queue full) is retried next scan.
            let usage = CONSUMER_USAGE.load(Ordering::Relaxed);
            if usage != last_wls_usage {
                defmt::debug!("wls extra: usage={=u16:#06x}", usage);
                if wireless::send_consumer(usage) {
                    last_wls_usage = usage;
                }
            }

            // Mouse over the radio (the vendor `md_send_mouse`, 0xA8). The
            // accelerator turns the held mouse keys into this tick's signed deltas
            // exactly as the USB path does; the 5-byte vendor frame carries the
            // vertical wheel (keeberry has no horizontal pan, so its byte is 0).
            // Movement/wheel are relative deltas (always emitted when non-zero);
            // buttons are absolute, so the cache advances only on a successful
            // enqueue and a drop (link down or queue full) is retried next scan.
            let mkeys = MOUSE_KEYS.load(Ordering::Relaxed);
            let (mx, my, mwheel) = mouse.step(mkeys, dt);
            let mbtns = mouse::buttons(mkeys);
            if (mx != 0 || my != 0 || mwheel != 0 || mbtns != last_mouse_buttons)
                && wireless::send_mouse(mbtns, mx, my, 0, mwheel)
            {
                last_mouse_buttons = mbtns;
            }
        }

        // Track the CONFIGURED edge every loop, regardless of transport (see
        // [`keyboard_loop`]).
        was_configured = configured;
        Timer::after(Duration::from_millis(1)).await;
    }
}

/// Serve the [`crate::kcp`] config protocol on the raw-HID vendor interface.
///
/// Blocks on the 32-byte interrupt OUT endpoint and, for each *complete*
/// 32-byte report, dispatches it through the pure [`kcp::handle`] and writes the
/// 32-byte reply back on the interrupt IN endpoint. A short report is dropped
/// without dispatch: this enforces the fixed 32-byte framing and — since `buf`
/// is reused across reads — prevents stale trailing bytes from a previous
/// request leaking in as the new request's SEQ/payload. Unlike [`keyboard_loop`],
/// no [`CONFIGURED`] gate is needed: the host only sends OUT reports once it has
/// configured the interface, so a completed read already implies the endpoints
/// are live and the reply can be sent.
///
/// On a read error — the OUT endpoint is disabled because the device is
/// unconfigured, or was just reset — it waits for the endpoint to be enabled again
/// before resuming, so a bus reset is handled transparently. Write errors are
/// dropped: if the host is not draining the IN endpoint there is nothing useful
/// to do with the reply.
async fn kcp_loop<'a>(
    reader: &mut HidReader<'a, Driver<'a>, KCP_REPORT_LEN>,
    writer: &mut HidWriter<'a, Driver<'a>, KCP_REPORT_LEN>,
) {
    let mut buf = [0u8; KCP_REPORT_LEN];
    loop {
        match reader.read(&mut buf).await {
            // A complete frame. `HidReader::read` returns the byte count and can
            // yield a short OUT report; only a full `KCP_REPORT_LEN` read has
            // freshly written every byte of `buf`, so this guard is what actually
            // enforces the 32-byte framing and keeps stale bytes out of `handle`.
            Ok(len) if len == KCP_REPORT_LEN => {
                let reply = kcp::handle(&buf);
                defmt::debug!(
                    "kcp: cmd={=u8:#04x} seq={=u8} -> status={=u8}",
                    buf[0],
                    buf[1],
                    reply[2]
                );
                // Best effort: drop the reply if the host is not draining IN.
                let _ = writer.write(&reply).await;
            }
            // Short / partial report: not a valid kcp message — ignore it.
            Ok(_) => {}
            // OUT endpoint not currently usable (unconfigured or just reset).
            // Wait for it to be enabled again, then resume reading.
            Err(_) => reader.ready().await,
        }
    }
}
