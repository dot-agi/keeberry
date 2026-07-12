// SPDX-License-Identifier: GPL-2.0-or-later
//! TIM2-based [`embassy-time`](https://docs.rs/embassy-time) driver for the
//! Westberry WB32FQ95.
//!
//! This registers the global [`embassy_time_driver::Driver`] backing
//! `embassy_time::Instant`/`Timer`. It is a faithful port of the proven
//! `embassy-stm32` 16-bit general-purpose timer driver
//! (`embassy-stm32/src/time_driver/gp16.rs`), adapted to this crate's
//! word-granular peripheral-access API. The WB32FQ95's TIM2 is a 16-bit
//! STM32F103-work-alike general-purpose timer, so the STM32 logic maps over
//! verbatim; only the register pokes differ.
//!
//! # Time base
//!
//! TIM2 is clocked from PCLK1 (96 MHz, see [`clock`](crate::clock)). The
//! prescaler divides this to a [`TICK_HZ`] = 1 MHz counter, and the auto-reload
//! is left at `0xFFFF` so the 16-bit counter free-runs and wraps every 65536
//! ticks (~65.5 ms).
//!
//! # 64-bit monotonic `now()` from a 16-bit counter
//!
//! Timekeeping works in "periods" of 2^15 ticks (half of the 16-bit counter
//! range). A [`Tim2Driver::period`] counter is maintained alongside the
//! hardware counter:
//!
//! * `period` and the hardware counter both start at 0.
//! * `period` is incremented on counter overflow (counter value 0), via the
//!   update interrupt (`UIF`).
//! * `period` is incremented "mid-way" between overflows (counter value
//!   `0x8000`), via the CC1 compare interrupt (`CC1IF`), whose `CCR1` is fixed
//!   at `0x8000`.
//!
//! Therefore the parity of `period` selects which half of the counter range is
//! live: even `period` => counter in `0..0x8000`, odd `period` => counter in
//! `0x8000..=0xFFFF`. [`calc_now`] folds the two together; reading `period`
//! before the counter, the parity correction makes the result correct even if
//! an overflow races the read. `period` is 32-bit, so the 64-bit tick value
//! wraps after `2^32` periods = `2^47` ticks, i.e. ~4.46 years of continuous
//! uptime at the 1 MHz tick. (The embassy-stm32 reference comment quotes ~136
//! years, but that is for its 32_768 Hz RTC tick — `2^47 / 32768` seconds; our
//! 1 MHz tick is ~30x faster, so `2^47 / 1_000_000` seconds ≈ 4.46 years.)
//!
//! # Half-period servicing invariant
//!
//! The scheme is only correct if TIM2's interrupt is serviced within roughly
//! half a period (~32 ms) of each (half-)overflow. `period` advances one step
//! per `UIF`/`CC1IF`, and those status flags are single-bit and sticky: if the
//! interrupt stays masked across more than one half-period boundary, the
//! intervening (half-)overflows collapse into a single flag, `period`
//! under-counts, and thereafter [`now`](Driver::now) is wrong — it can even
//! momentarily step backwards once the handler catches up — and a deferred
//! alarm (see below) can be delayed by up to one full counter wrap (~65 ms).
//! This is an inherent property of the ported embassy-stm32 `gp16.rs` design,
//! not a keeberry-specific limitation. keeberry never masks interrupts anywhere
//! near 32 ms — its longest critical sections are a handful of register writes —
//! so the invariant always holds.
//!
//! # Alarms
//!
//! CC2 (compare register `CCR2`, interrupt `CC2IF`) provides the single alarm
//! that `embassy-time` requires. The pending wakers live in an
//! [`embassy_time_queue_utils::Queue`]; [`Driver::schedule_wake`] enqueues a
//! waker and arms CC2 for the earliest deadline, and the interrupt handler
//! drains expired entries and re-arms for the next one. To avoid arming a
//! compare that is more than one period away (where the 16-bit `CCR2` would be
//! ambiguous), CC2 is only unmasked once the deadline is within `0xC000` ticks;
//! otherwise [`Tim2Driver::next_period`] unmasks it when the time draws near.
//!
//! [`on_interrupt`](Tim2Driver::on_interrupt) snapshots `DIER` once and fires
//! the alarm only when that snapshot already had CC2 enabled
//! (`CC2IF && CC2IE`) — identical to the reference (`gp16.rs` `on_interrupt`,
//! `if sr.ccif(n+1) && dier.ccie(n+1)`). Under the half-period invariant this
//! never drops a wake; only masking the handler across both a deferred alarm's
//! arming boundary and its deadline (the >32 ms case above) could let the stale
//! snapshot defer the wake by one counter wrap.

use core::cell::{Cell, RefCell};
use core::sync::atomic::{compiler_fence, AtomicU32, Ordering};
use core::task::Waker;

use critical_section::CriticalSection;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;
use embassy_time_driver::{Driver, TICK_HZ};
use embassy_time_queue_utils::Queue;
use pac::{Interrupt, Peripherals, Tim2};

use crate::clock;

// === RCC.APB1ENR: TIM2 peripheral clock enable ===
/// RCC.APB1ENR: TIM2 clock enable
/// (`RCC_APB1ENR_TIM2EN = 0x1U << 2`, wb32fq95xx.h:4150).
const RCC_APB1ENR_TIM2EN: u32 = 0x1 << 2;

// === TIM.CR1: control register 1 ===
/// TIM.CR1: counter enable (`TIM_CR1_CEN = 0x1U << 0`, wb32fq95xx.h:2102).
const TIM_CR1_CEN: u32 = 0x1 << 0;
/// TIM.CR1: update request source — when set, a forced update (UG) does not
/// raise `UIF` (`TIM_CR1_URS = 0x1U << 2`, wb32fq95xx.h:2104).
const TIM_CR1_URS: u32 = 0x1 << 2;

// === TIM.DIER: DMA/interrupt enable register ===
/// TIM.DIER: update interrupt enable (`TIM_DIER_UIE = 0x1U << 0`, wb32fq95xx.h:2164).
const TIM_DIER_UIE: u32 = 0x1 << 0;
/// TIM.DIER: capture/compare 1 interrupt enable — the half-overflow marker
/// (`TIM_DIER_CC1IE = 0x1U << 1`, wb32fq95xx.h:2165).
const TIM_DIER_CC1IE: u32 = 0x1 << 1;
/// TIM.DIER: capture/compare 2 interrupt enable — the alarm
/// (`TIM_DIER_CC2IE = 0x1U << 2`, wb32fq95xx.h:2166).
const TIM_DIER_CC2IE: u32 = 0x1 << 2;

// === TIM.CCER: capture/compare enable register ===
/// TIM.CCER: capture/compare 1 output enable (`WB32_TIM_CCER_CC1E = 1U << 0`).
/// On the WB32 TIM a compare match raises `CCxIF` ONLY when the channel is
/// enabled here; CC1 (the half-overflow marker) needs it set.
const TIM_CCER_CC1E: u32 = 0x1 << 0;
/// TIM.CCER: capture/compare 2 output enable (`WB32_TIM_CCER_CC2E = 1U << 4`).
/// CC2 (the alarm) needs it set to raise `CC2IF` on its compare match.
const TIM_CCER_CC2E: u32 = 0x1 << 4;

// === TIM.SR: status register (flags are read / write-0-to-clear) ===
/// TIM.SR: update interrupt flag (`TIM_SR_UIF = 0x1U << 0`, wb32fq95xx.h:2181).
const TIM_SR_UIF: u32 = 0x1 << 0;
/// TIM.SR: capture/compare 1 interrupt flag (`TIM_SR_CC1IF = 0x1U << 1`, wb32fq95xx.h:2182).
const TIM_SR_CC1IF: u32 = 0x1 << 1;
/// TIM.SR: capture/compare 2 interrupt flag (`TIM_SR_CC2IF = 0x1U << 2`, wb32fq95xx.h:2183).
const TIM_SR_CC2IF: u32 = 0x1 << 2;

// === TIM.EGR: event generation register ===
/// TIM.EGR: update generation — force an update event to load the prescaler
/// (`TIM_EGR_UG = 0x1U << 0`, wb32fq95xx.h:2195).
const TIM_EGR_UG: u32 = 0x1 << 0;

/// CC1 compare value marking the mid-way (half-overflow) point of the counter.
const HALF_PERIOD: u32 = 0x8000;
/// Auto-reload value: free-run the full 16-bit range.
const ARR_FULL: u32 = 0xFFFF;
/// A deadline closer than this many ticks is armed immediately; anything
/// further is deferred to [`Tim2Driver::next_period`]. Matches the reference's
/// `0xc000` guard band.
const ALARM_ARM_WINDOW: u64 = 0xC000;

/// Prescaler for a 1 MHz tick: `PCLK1 / TICK_HZ - 1` (96 MHz / 1 MHz - 1 = 95).
const PSC: u32 = clock::HCLK_HZ / (TICK_HZ as u32) - 1;
const _: () = assert!(PSC <= 0xFFFF, "TIM2 prescaler does not fit in 16 bits");

/// Pointer to TIM2's register block.
///
/// TIM2 is owned by the [`Driver`] after [`init`]; the interrupt handler and
/// the `Driver` methods reach it by stealing, coordinating all shared mutable
/// state through critical sections.
#[inline]
fn regs() -> Tim2 {
    // SAFETY: TIM2 is configured once in `init` and thereafter only accessed
    // here; mutations of shared state happen inside critical sections.
    unsafe { Tim2::steal() }
}

/// Fold the software `period` and the 16-bit hardware `counter` into a 64-bit
/// monotonic tick count. See the module documentation for the race argument.
fn calc_now(period: u32, counter: u16) -> u64 {
    ((period as u64) << 15) + ((counter as u32 ^ ((period & 1) << 15)) as u64)
}

/// The single pending alarm deadline, in ticks (`u64::MAX` when disarmed).
struct AlarmState {
    timestamp: Cell<u64>,
}

// SAFETY: `AlarmState` is only ever accessed inside critical sections, which
// provide the required mutual exclusion on this single-core target.
unsafe impl Send for AlarmState {}

impl AlarmState {
    const fn new() -> Self {
        Self {
            timestamp: Cell::new(u64::MAX),
        }
    }
}

/// TIM2-backed `embassy-time` driver.
struct Tim2Driver {
    /// Number of elapsed 2^15-tick periods since [`init`].
    period: AtomicU32,
    /// The armed alarm deadline.
    alarm: Mutex<CriticalSectionRawMutex, AlarmState>,
    /// Wakers waiting on time, ordered by deadline.
    queue: Mutex<CriticalSectionRawMutex, RefCell<Queue>>,
}

embassy_time_driver::time_driver_impl!(static DRIVER: Tim2Driver = Tim2Driver {
    period: AtomicU32::new(0),
    alarm: Mutex::const_new(CriticalSectionRawMutex::new(), AlarmState::new()),
    queue: Mutex::new(RefCell::new(Queue::new())),
});

impl Tim2Driver {
    /// Configure TIM2 as the 1 MHz time base and start it. Called once from
    /// [`init`].
    fn start(&'static self, p: &Peripherals) {
        // Enable the TIM2 peripheral clock on APB1; the read-back is a write
        // barrier, mirroring the vendor `rccEnableAPB1` dummy read.
        p.rcc
            .apb1enr()
            .modify(|r, w| unsafe { w.bits(r.bits() | RCC_APB1ENR_TIM2EN) });
        let _ = p.rcc.apb1enr().read().bits();

        let r = regs();

        // Stop and zero the counter while reconfiguring.
        r.cr1().modify(|r, w| unsafe { w.bits(r.bits() & !TIM_CR1_CEN) });
        r.cnt().write(|w| unsafe { w.bits(0) });

        // 1 MHz tick rate, full 16-bit auto-reload.
        r.psc().write(|w| unsafe { w.bits(PSC) });
        r.arr().write(|w| unsafe { w.bits(ARR_FULL) });

        // Set URS, force an update to latch the prescaler, then clear URS. With
        // URS set the forced update reloads PSC without raising UIF, so no
        // spurious overflow interrupt is left pending.
        r.cr1().modify(|r, w| unsafe { w.bits(r.bits() | TIM_CR1_URS) });
        r.egr().write(|w| unsafe { w.bits(TIM_EGR_UG) });
        r.cr1().modify(|r, w| unsafe { w.bits(r.bits() & !TIM_CR1_URS) });

        // CC1 marks the half-overflow point; CC2's interrupt (CC2IE) is left
        // masked until an alarm is armed.
        r.ccr1().write(|w| unsafe { w.bits(HALF_PERIOD) });
        r.dier()
            .write(|w| unsafe { w.bits(TIM_DIER_UIE | TIM_DIER_CC1IE) });
        // Enable both compare channels in CCER. CRITICAL on the WB32 TIM: a
        // compare match raises `CCxIF` only when the channel is enabled here (the
        // donor's ChibiOS `hal_st_lld.h:228,307` does exactly this alongside
        // `DIER`). The channels stay in their reset output/frozen mode with no pin
        // alternate-function, so there is no physical output — only the flags. Omit
        // this and neither CC1 (half-overflow) nor CC2 (alarm) ever fires: `now()`
        // then advances on full overflow only (half rate) and every `embassy-time`
        // timer is dilated ~1000×, silently killing the matrix scan and RGB while
        // the interrupt-driven USB/kcp path stays healthy.
        r.ccer()
            .write(|w| unsafe { w.bits(TIM_CCER_CC1E | TIM_CCER_CC2E) });

        // Route TIM2 (IRQ 21) through the NVIC.
        cortex_m::peripheral::NVIC::unpend(Interrupt::TIM2);
        // SAFETY: enabling the timer interrupt cannot break any
        // mask-based critical section once the driver is initialised.
        unsafe { cortex_m::peripheral::NVIC::unmask(Interrupt::TIM2) };

        // Start counting.
        r.cr1().modify(|r, w| unsafe { w.bits(r.bits() | TIM_CR1_CEN) });
    }

    /// TIM2 interrupt service routine: advance the period on (half-)overflow and
    /// fire a due alarm.
    fn on_interrupt(&self) {
        let r = regs();

        critical_section::with(|cs| {
            let sr = r.sr().read().bits();
            let dier = r.dier().read().bits();

            // SR flags are write-0-to-clear: write the bitwise NOT so only the
            // flags we observed are cleared, never one set between read and
            // write.
            r.sr().write(|w| unsafe { w.bits(!sr) });

            // Overflow and half-overflow both advance the period.
            if sr & TIM_SR_UIF != 0 {
                self.next_period();
            }
            if sr & TIM_SR_CC1IF != 0 {
                self.next_period();
            }

            // CC2: the alarm, only if it is currently armed.
            if (sr & TIM_SR_CC2IF != 0) && (dier & TIM_DIER_CC2IE != 0) {
                self.trigger_alarm(cs);
            }
        });
    }

    /// Increment the period counter and, if the armed alarm now falls within the
    /// arming window, unmask CC2 (its `CCR2` was set when the alarm was armed).
    fn next_period(&self) {
        let r = regs();

        // The period is only ever modified from the interrupt, so this is race
        // free. `wrapping_add` so an overflow-checked (debug) build cannot panic
        // in the ISR at the ~4.46-year period wrap; the value is used modulo
        // 2^32 anyway, since the high bits fall off the `<< 15` in `calc_now`.
        let period = self.period.load(Ordering::Relaxed).wrapping_add(1);
        self.period.store(period, Ordering::Relaxed);
        let t = (period as u64) << 15;

        critical_section::with(move |cs| {
            let at = self.alarm.borrow(cs).timestamp.get();
            if at < t + ALARM_ARM_WINDOW {
                // Within range: enable CC2. `set_alarm` already wrote CCR2.
                r.dier()
                    .modify(|r, w| unsafe { w.bits(r.bits() | TIM_DIER_CC2IE) });
            }
        });
    }

    /// Drain expired wakers and re-arm CC2 for the next deadline.
    fn trigger_alarm(&self, cs: CriticalSection) {
        let mut next = self.queue.borrow(cs).borrow_mut().next_expiration(self.now());
        while !self.set_alarm(cs, next) {
            next = self.queue.borrow(cs).borrow_mut().next_expiration(self.now());
        }
    }

    /// Arm CC2 for `timestamp`. Returns `false` if the deadline has already
    /// passed (the caller must then re-query the queue for the next one).
    fn set_alarm(&self, cs: CriticalSection, timestamp: u64) -> bool {
        let r = regs();

        self.alarm.borrow(cs).timestamp.set(timestamp);

        let t = self.now();
        if timestamp <= t {
            // Already past: disarm and report failure.
            r.dier()
                .modify(|r, w| unsafe { w.bits(r.bits() & !TIM_DIER_CC2IE) });
            self.alarm.borrow(cs).timestamp.set(u64::MAX);
            return false;
        }

        // Write CCR2 regardless; `next_period` may unmask it later.
        r.ccr2().write(|w| unsafe { w.bits((timestamp as u16) as u32) });

        // Only unmask now if it will ring within one window; otherwise
        // `next_period` arms it when the time approaches.
        let diff = timestamp - t;
        r.dier().modify(|r, w| unsafe {
            let bits = r.bits();
            w.bits(if diff < ALARM_ARM_WINDOW {
                bits | TIM_DIER_CC2IE
            } else {
                bits & !TIM_DIER_CC2IE
            })
        });

        // Re-check for a deadline that slipped into the past while arming.
        let t = self.now();
        if timestamp <= t {
            r.dier()
                .modify(|r, w| unsafe { w.bits(r.bits() & !TIM_DIER_CC2IE) });
            self.alarm.borrow(cs).timestamp.set(u64::MAX);
            return false;
        }

        true
    }
}

impl Driver for Tim2Driver {
    fn now(&self) -> u64 {
        let r = regs();

        let period = self.period.load(Ordering::Relaxed);
        compiler_fence(Ordering::Acquire);
        let counter = r.cnt().read().bits() as u16;
        calc_now(period, counter)
    }

    fn schedule_wake(&self, at: u64, waker: &Waker) {
        critical_section::with(|cs| {
            let mut queue = self.queue.borrow(cs).borrow_mut();

            if queue.schedule_wake(at, waker) {
                let mut next = queue.next_expiration(self.now());
                while !self.set_alarm(cs, next) {
                    next = queue.next_expiration(self.now());
                }
            }
        });
    }
}

/// Initialise the TIM2 time base and register it as the global
/// `embassy-time` driver. Must be called once, after [`clock::init`] and before
/// the first use of `embassy_time`.
pub fn init(p: &Peripherals) {
    DRIVER.start(p);
}

/// TIM2 global interrupt (IRQ 21).
///
/// The PAC does not re-export cortex-m-rt's `#[interrupt]` attribute, so the
/// handler is provided directly as the `TIM2` symbol, overriding the weak
/// `PROVIDE(TIM2 = DefaultHandler)` from the PAC's `device.x`. This is exactly
/// what `#[interrupt] fn TIM2()` expands to.
#[no_mangle]
extern "C" fn TIM2() {
    DRIVER.on_interrupt();
}
