// SPDX-License-Identifier: GPL-2.0-or-later
//! Interrupt-driven async UART3 driver for the Westberry WB32FQ95.
//!
//! UART3 is the wire to the CH582F BLE/2.4G radio. The radio link runs at
//! **115200 8N1, no flow control**, on **PC10 (TX) / PC11 (RX), alternate
//! function 7** — exactly the QMK Akko 5075B configuration
//! (`vial-qmk keyboards/akko/5075b/ansi/config.h:76-80`,
//! `mcuconf.h` `WB32_SERIAL_USE_UART3 TRUE`).
//!
//! The register bring-up is a faithful port of the ChibiOS-Contrib WB32 serial
//! LLD (`os/hal/ports/WB32/LLD/UARTv1/hal_serial_lld.c`, `uart_init` and
//! `sd_lld_start`); every field value cites its line in that LLD or in the
//! vendor CMSIS header `os/common/ext/CMSIS/WB32/WB32FQ95xx/wb32fq95xx.h`
//! (abbreviated `wb32fq95xx.h` below). The WB32 UART is a Synopsys DesignWare
//! APB core (note the fractional `DLF` divisor and the `SFE`/`SRT` shadow
//! registers), so the 16550-style programming model maps over verbatim.
//!
//! # Async model
//!
//! Both directions are interrupt-driven through the single [`UART3`] vector
//! (IRQ 32, `wb32_isr.h:182-183` `WB32_UART3_NUMBER 32`), mirroring the
//! interrupt+waker pattern of [`crate::time_driver`]:
//!
//! * **RX** — the ISR drains the receive FIFO into [`RX_RING`] and wakes
//!   [`RX_WAKER`]; [`read`] awaits a byte, [`try_read`] polls.
//! * **TX** — [`write_all`] enqueues bytes into [`TX_RING`] and arms the
//!   transmit-holding-empty interrupt; the ISR feeds one byte per `THRE` event
//!   (matching the LLD `serve_interrupt`/`notify3`) and masks the interrupt when
//!   the ring drains, waking [`TX_WAKER`] so a blocked writer can refill.
//!
//! The ring/interrupt-enable updates are done inside `critical_section` blocks
//! so the push+arm and the pop+mask pairs cannot interleave, which is what
//! prevents the classic "byte left in the ring with `THRE` masked" TX stall.

use core::cell::RefCell;
use core::future::poll_fn;
use core::task::Poll;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_sync::mutex::Mutex as AsyncMutex;
use embassy_sync::waitqueue::AtomicWaker;
use pac::{Interrupt, Peripherals, Uart3};

use crate::clock;
use crate::gpio::{self, Pin, Port};

// === Pin map (vial-qmk keyboards/akko/5075b/ansi/config.h:76-80) ===
/// UART3 TX: PC10, alternate function 7.
const TX_PIN: Pin = Pin::new(Port::C, 10);
/// UART3 RX: PC11, alternate function 7.
const RX_PIN: Pin = Pin::new(Port::C, 11);
/// Alternate function selecting UART3 on PC10/PC11 (`UART_TX_PAL_MODE 7`).
const UART_AF: u8 = 7;

// === Baud divisor (hal_serial_lld.c:88-95) ===
// The LLD computes `divider = (apbclock + speed/2) / speed`, then splits it into
// the integer latch DLH:DLL (`divider >> 4`) and the 4-bit fractional latch DLF
// (`divider & 0x0F`); the effective baud is `apbclock / divider`. UART3 is on
// APB2, whose clock equals HCLK here because the APB2 prescaler is /1
// (see [`crate::clock`]): PCLK2 = 96 MHz.
/// Target baud rate (`MD_BAUD_RATE`, smsg.c:10).
const BAUD: u32 = 115_200;
/// UART3 source clock = PCLK2 = HCLK (APB2 prescaler /1).
const PCLK2_HZ: u32 = clock::HCLK_HZ;
/// Rounded divisor `(PCLK2 + BAUD/2) / BAUD` = 833 → 96 MHz / 833 ≈ 115246 baud
/// (0.04% error, well inside UART tolerance).
const DIVIDER: u32 = (PCLK2_HZ + BAUD / 2) / BAUD;
/// Divisor latch low byte (`divider >> 4`).
const DLL_VAL: u32 = (DIVIDER >> 4) & 0xFF;
/// Divisor latch high byte (`divider >> 12`).
const DLH_VAL: u32 = (DIVIDER >> 12) & 0xFF;
/// Fractional divisor latch (`divider & 0x0F`).
const DLF_VAL: u32 = DIVIDER & 0x0F;

// === UART LCR (line control) bits (wb32fq95xx.h) ===
/// Divisor Latch Access Bit (`UART_LCR_DLAB = 0x1U << 7`, wb32fq95xx.h:1407).
const UART_LCR_DLAB: u32 = 0x1 << 7;
/// 8-bit word length (`UART_LCR_WLS_8BIT = 0x3U << 0`, wb32fq95xx.h:1384).
const UART_LCR_WLS_8BIT: u32 = 0x3 << 0;
/// 1 stop bit (`UART_LCR_SBS_1BIT = 0x0U << 2`, wb32fq95xx.h:1387).
const UART_LCR_SBS_1BIT: u32 = 0x0 << 2;
/// No parity (`UART_LCR_PARITY_NONE = 0x0U << 3`, wb32fq95xx.h:1399).
const UART_LCR_PARITY_NONE: u32 = 0x0 << 3;

// === UART IER (interrupt enable) bits (wb32fq95xx.h) ===
/// Received-data-available interrupt enable
/// (`UART_IER_RDAIE = 0x1U << 0`, wb32fq95xx.h:1341).
const UART_IER_RDAIE: u32 = 0x1 << 0;
/// Transmit-holding-register-empty interrupt enable
/// (`UART_IER_THREIE = 0x1U << 1`, wb32fq95xx.h:1342).
const UART_IER_THREIE: u32 = 0x1 << 1;
/// Receiver-line-status interrupt enable
/// (`UART_IER_RLSIE = 0x1U << 2`, wb32fq95xx.h:1343).
const UART_IER_RLSIE: u32 = 0x1 << 2;

// === UART LSR (line status) bits (wb32fq95xx.h) ===
/// Data Ready: at least one character is in the RX FIFO
/// (`UART_LSR_DR = 0x1U << 0`, wb32fq95xx.h:1417).
const UART_LSR_DR: u32 = 0x1 << 0;
/// Transmit Holding Register Empty
/// (`UART_LSR_THRE = 0x1U << 5`, wb32fq95xx.h:1422).
const UART_LSR_THRE: u32 = 0x1 << 5;

// === UART FIFO / MCR programming (hal_serial_lld.c) ===
/// RX FIFO trigger level = 1 character, written to the shadow `SRT` register
/// (`UART_RxFIFOThreshold_1 = 0x00`, hal_serial_lld.h:88; used at
/// hal_serial_lld.c:101).
const UART_RX_FIFO_THRESHOLD_1: u32 = 0x00;
/// Shadow FIFO Enable value turning the FIFO on
/// (`u->SFE = 0x01`, hal_serial_lld.c:103).
const UART_SFE_ENABLE: u32 = 0x01;
/// Bits of `MCR` the LLD preserves while (re)writing flow control
/// (`u->MCR = (u->MCR & 0x50) | ...`, hal_serial_lld.c:78).
const UART_MCR_KEEP_MASK: u32 = 0x50;
/// Auto-flow-control disabled (`UART_AutoFlowControl_None = 0x00`,
/// hal_serial_lld.h:77; the 5th `SerialConfig` field, default_config line 61).
const UART_AUTOFLOW_NONE: u32 = 0x00;

// === RCC clock-enable / reset bits (wb32fq95xx.h) ===
/// RCC.APB2ENR UART3 clock enable
/// (`RCC_APB2ENR_UART3EN = 0x1U << 3`, wb32fq95xx.h:4169; `rccEnableUART3`,
/// wb32_rcc.h:535).
const RCC_APB2ENR_UART3EN: u32 = 0x1 << 3;
/// RCC.APB2RSTR UART3 peripheral reset
/// (`RCC_APB2RSTR_UART3RST = 0x1U << 3`, wb32fq95xx.h:4232; `rccResetUART3`,
/// wb32_rcc.h:549).
const RCC_APB2RSTR_UART3RST: u32 = 0x1 << 3;
/// RCC.APB1ENR GPIOC clock enable; PC10/PC11 carry UART3
/// (`RCC_APB1ENR_GPIOCEN = 0x1U << 7`, wb32fq95xx.h:4155). Enabled defensively;
/// [`crate::matrix`] also turns GPIOC on.
const RCC_APB1ENR_GPIOCEN: u32 = 0x1 << 7;

/// RX ring capacity (bytes). One radio frame is at most 36 bytes, so this holds
/// several in-flight frames without dropping under a burst.
const RX_RING_LEN: usize = 64;
/// TX ring capacity (bytes). Sized past the largest single frame (the 36-byte
/// raw-HID frame) so [`write_all`] never blocks mid-frame in practice.
const TX_RING_LEN: usize = 64;

/// A single-producer/single-consumer byte ring. All access is wrapped in a
/// `critical_section` by the callers, so the methods themselves need no locking.
struct Ring<const N: usize> {
    buf: [u8; N],
    head: usize,
    len: usize,
}

impl<const N: usize> Ring<N> {
    const fn new() -> Self {
        Self {
            buf: [0; N],
            head: 0,
            len: 0,
        }
    }

    /// Append a byte; returns `false` (dropping the byte) when full.
    fn push(&mut self, b: u8) -> bool {
        if self.len >= N {
            return false;
        }
        let i = (self.head + self.len) % N;
        self.buf[i] = b;
        self.len += 1;
        true
    }

    /// Remove and return the oldest byte, or `None` when empty.
    fn pop(&mut self) -> Option<u8> {
        if self.len == 0 {
            return None;
        }
        let b = self.buf[self.head];
        self.head = (self.head + 1) % N;
        self.len -= 1;
        Some(b)
    }
}

/// Bytes received by the ISR, awaiting [`read`].
static RX_RING: Mutex<CriticalSectionRawMutex, RefCell<Ring<RX_RING_LEN>>> =
    Mutex::new(RefCell::new(Ring::new()));
/// Bytes queued by [`write_all`], drained by the ISR into `THR`.
static TX_RING: Mutex<CriticalSectionRawMutex, RefCell<Ring<TX_RING_LEN>>> =
    Mutex::new(RefCell::new(Ring::new()));
/// Woken when the ISR delivers RX data.
static RX_WAKER: AtomicWaker = AtomicWaker::new();
/// Woken when the ISR frees TX ring space.
static TX_WAKER: AtomicWaker = AtomicWaker::new();

/// Serialises whole-frame TX writes. Two async tasks share the UART3
/// transmitter — the RX pump's 3-byte ACK echo and the stop-and-wait `tx_task` —
/// and [`write_all`] parks on the single [`TX_WAKER`]. Were the ring to fill
/// while both are mid-write, they would overwrite each other's registered waker
/// (a lost wakeup stranding one writer) and interleave their bytes (corrupting
/// the ACK or the frame). Holding this async mutex across the whole of
/// [`write_all`] makes each frame/ACK atomic and guarantees only one waiter is
/// ever registered on [`TX_WAKER`]. The ISR never touches this lock — it
/// coordinates with the writers through the `critical_section`-guarded ring —
/// so there is no ISR/task deadlock.
static TX_LOCK: AsyncMutex<CriticalSectionRawMutex, ()> = AsyncMutex::new(());

/// UART3 register block.
///
/// UART3 is configured once in [`init`] and thereafter reached by stealing from
/// both task context and the ISR; all shared software state lives in the
/// `critical_section`-guarded rings, and the hardware registers tolerate the
/// concurrent volatile access.
#[inline]
fn regs() -> Uart3 {
    // SAFETY: the register block is memory-mapped and valid for the whole
    // program; every access goes through the PAC's volatile proxies.
    unsafe { Uart3::steal() }
}

/// Bring UART3 up: PC10/PC11 to AF7, clock + reset the peripheral, program
/// 115200 8N1 with the FIFO on, enable RX interrupts, and route IRQ 32.
///
/// Port of the LLD `sd_lld_start` + `uart_init` (hal_serial_lld.c:343-378,
/// 74-110). Must run after [`crate::clock::init`].
pub fn init(p: &Peripherals) {
    // Ensure GPIOC is clocked, then route PC10/PC11 to UART3 (AF7). The
    // read-back is the write barrier the vendor `rccEnableAPB1` performs.
    p.rcc
        .apb1enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB1ENR_GPIOCEN) });
    let _ = p.rcc.apb1enr().read().bits();
    gpio::set_alternate_push_pull(TX_PIN, UART_AF);
    gpio::set_alternate_push_pull(RX_PIN, UART_AF);

    // rccEnableUART3(): enable the UART3 clock on APB2 (read-back barrier).
    p.rcc
        .apb2enr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB2ENR_UART3EN) });
    let _ = p.rcc.apb2enr().read().bits();
    // rccResetUART3(): pulse the peripheral reset (set then clear).
    p.rcc
        .apb2rstr()
        .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB2RSTR_UART3RST) });
    p.rcc
        .apb2rstr()
        .modify(|r, w| unsafe { w.bits(r.bits() & !RCC_APB2RSTR_UART3RST) });

    let u = regs();

    // MCR = (MCR & 0x50) | AutoFlowControl(None) (hal_serial_lld.c:78).
    u.mcr()
        .modify(|r, w| unsafe { w.bits((r.bits() & UART_MCR_KEEP_MASK) | UART_AUTOFLOW_NONE) });

    // Program the baud divisor (hal_serial_lld.c:91-95). DLF is independent of
    // DLAB; DLL/DLH share their addresses with THR/RBR/IER and are only
    // reachable while DLAB is latched.
    u.dlf().write(|w| unsafe { w.bits(DLF_VAL) });
    u.lcr().write(|w| unsafe { w.bits(UART_LCR_DLAB) });
    u.dll().write(|w| unsafe { w.bits(DLL_VAL) });
    u.dlh().write(|w| unsafe { w.bits(DLH_VAL) });
    u.lcr().write(|w| unsafe { w.bits(0) });

    // 8 data bits, 1 stop bit, no parity (hal_serial_lld.c:97-99).
    u.lcr()
        .write(|w| unsafe { w.bits(UART_LCR_WLS_8BIT | UART_LCR_SBS_1BIT | UART_LCR_PARITY_NONE) });

    // RX FIFO trigger = 1 char, FIFO enabled (hal_serial_lld.c:101-103).
    u.srt().write(|w| unsafe { w.bits(UART_RX_FIFO_THRESHOLD_1) });
    u.sfe().write(|w| unsafe { w.bits(UART_SFE_ENABLE) });

    // Enable RX-data-available + RX-line-status interrupts; THRE is armed
    // on demand by `write_all` (hal_serial_lld.c:105).
    u.ier()
        .modify(|r, w| unsafe { w.bits(r.bits() | UART_IER_RDAIE | UART_IER_RLSIE) });

    // Route UART3 (IRQ 32) through the NVIC. The vendor mcuconf sets priority 8
    // (`WB32_SERIAL_UART3_PRIORITY 8`); priority tuning is left to the default
    // here, as for the TIM2 driver.
    cortex_m::peripheral::NVIC::unpend(Interrupt::UART3);
    // SAFETY: enabling the UART3 interrupt cannot break a mask-based critical
    // section now that the driver is initialised.
    unsafe { cortex_m::peripheral::NVIC::unmask(Interrupt::UART3) };
}

/// Await and return one received byte.
pub async fn read() -> u8 {
    poll_fn(|cx| {
        // Register before checking: a byte the ISR delivers after the check but
        // before we sleep still wakes us.
        RX_WAKER.register(cx.waker());
        match try_read() {
            Some(b) => Poll::Ready(b),
            None => Poll::Pending,
        }
    })
    .await
}

/// Non-blocking single-byte read; `None` when no byte is buffered.
pub fn try_read() -> Option<u8> {
    critical_section::with(|cs| RX_RING.borrow(cs).borrow_mut().pop())
}

/// Queue every byte of `data` for transmission, awaiting ring space when full.
///
/// The whole write is serialised by [`TX_LOCK`] so concurrent callers (the ACK
/// echo and the frame `tx_task`) never interleave bytes or clobber the shared
/// [`TX_WAKER`]. The bytes leave asynchronously under the ISR; this is the
/// `sdWrite` equivalent the radio codec's `uart_transmit` builds on
/// (uart_serial.c:138).
pub async fn write_all(data: &[u8]) {
    // Held for the entire frame/ACK: see [`TX_LOCK`].
    let _tx = TX_LOCK.lock().await;
    for &b in data {
        poll_fn(|cx| {
            TX_WAKER.register(cx.waker());
            // Push the byte and arm THRE atomically, so the ISR can never pop the
            // ring empty and mask THRE between our push and our arm (which would
            // strand the byte with no interrupt to drain it).
            let pushed = critical_section::with(|cs| {
                if TX_RING.borrow(cs).borrow_mut().push(b) {
                    regs()
                        .ier()
                        .modify(|r, w| unsafe { w.bits(r.bits() | UART_IER_THREIE) });
                    true
                } else {
                    false
                }
            });
            if pushed {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        })
        .await;
    }
}

/// UART3 global interrupt (IRQ 32).
///
/// Overrides the PAC's weak `UART3` vector, exactly as `#[interrupt] fn UART3()`
/// would expand. Drains RX into [`RX_RING`], then feeds one queued byte to `THR`
/// per `THRE` (matching `serve_interrupt`, hal_serial_lld.c:167-219).
#[no_mangle]
extern "C" fn UART3() {
    let u = regs();

    // RX: drain the FIFO while data is ready. Reading LSR also clears the
    // sticky RX error bits (OE/PE/FE/BI) raised by the RLS interrupt, so a
    // line-status event without data still deasserts the interrupt.
    loop {
        let lsr = u.lsr().read().bits();
        if lsr & UART_LSR_DR == 0 {
            break;
        }
        let b = (u.rbr().read().bits() & 0xFF) as u8;
        critical_section::with(|cs| {
            // Overrun drops the byte, like the hardware FIFO would.
            let _ = RX_RING.borrow(cs).borrow_mut().push(b);
        });
    }
    RX_WAKER.wake();

    // TX: if THRE is armed and the holding register is empty, feed one byte or
    // mask THRE when the ring is empty. Pop + mask are atomic against
    // `write_all`'s push + arm.
    let ier = u.ier().read().bits();
    if ier & UART_IER_THREIE != 0 && u.lsr().read().bits() & UART_LSR_THRE != 0 {
        critical_section::with(|cs| match TX_RING.borrow(cs).borrow_mut().pop() {
            Some(b) => u.thr().write(|w| unsafe { w.bits(b as u32) }),
            None => u
                .ier()
                .modify(|r, w| unsafe { w.bits(r.bits() & !UART_IER_THREIE) }),
        });
        TX_WAKER.wake();
    }
}
