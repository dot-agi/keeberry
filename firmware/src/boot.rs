// SPDX-License-Identifier: GPL-2.0-or-later
//! Software entry into the Westberry `wb32-dfu` bootloader.
//!
//! The WB32FQ95 ships a ROM bootloader at [`WB32_BOOTLOADER_ADDRESS`] that the
//! `wb32-dfu` flashing tool talks to. The stock hardware way to reach it is to
//! hold the bootloader key (Esc) while plugging the board in; this module adds a
//! software path so a kcp command (a GUI button) or a [`BOOTLOADER`] keypress (a Fn
//! combo) can re-enter DFU without the cable dance.
//!
//! [`BOOTLOADER`]: crate::keycode::BOOTLOADER
//!
//! # Mechanism (a faithful port of QMK)
//!
//! This is a direct port of QMK's
//! `platforms/chibios/bootloaders/wb32_dfu.c`. A magic word is written to a
//! fixed cell at the very top of SRAM and the MCU is reset; the reset path then,
//! before any RAM is initialised, sees the magic and jumps to the ROM
//! bootloader's reset vector instead of starting the firmware:
//!
//! ```c
//! extern uint32_t __ram0_end__;
//! #define BOOTLOADER_MAGIC 0xDEADBEEF
//! #define MAGIC_ADDR (unsigned long *)(SYMVAL(__ram0_end__) - 4)
//!
//! void bootloader_jump(void) {
//!     *MAGIC_ADDR = BOOTLOADER_MAGIC;
//!     NVIC_SystemReset();
//! }
//!
//! void enter_bootloader_mode_if_requested(void) {
//!     unsigned long *check = MAGIC_ADDR;
//!     if (*check == BOOTLOADER_MAGIC) {
//!         *check = 0;
//!         __set_CONTROL(0);
//!         __set_MSP(*(__IO uint32_t *)WB32_BOOTLOADER_ADDRESS);
//!         __enable_irq();
//!         ((void(*)(void)) *(uint32_t*)(WB32_BOOTLOADER_ADDRESS + 4))();
//!         while (1);
//!     }
//! }
//! ```
//!
//! # The magic word survives the reset
//!
//! [`MAGIC_ADDR`] is `0x2000_6FFC`, the last word of the chip's 28 KiB SRAM
//! (`0x2000_0000 + 28K - 4`), i.e. QMK's `__ram0_end__ - 4`. `firmware/memory.x`
//! shortens the linker's `RAM` region to `28K - 8` so this word lies *outside*
//! every section the toolchain places (`.data`/`.bss`/`.uninit` all live in
//! `RAM`) and *above* `_stack_start` (which cortex-m-rt sets to
//! `ORIGIN(RAM) + LENGTH(RAM)` = `0x2000_6FF8`). The main stack is full
//! descending, so its first push lands at `_stack_start - 4` = `0x2000_6FF4`;
//! the magic cell is never touched by the stack either. (Eight bytes, not four,
//! are reserved so `_stack_start` stays 8-byte aligned, which AAPCS requires and
//! cortex-m-rt enforces with `ASSERT(_stack_start % 8 == 0)`; the word below the
//! magic, `0x2000_6FF8`, is alignment padding.) Because the cell belongs to no
//! initialised section, the `Reset` handler's `.bss`/`.data` setup leaves it
//! intact, so a value written just before the reset is still there when
//! [`enter_bootloader_mode_if_requested`] reads it.

use cortex_m::peripheral::SCB;

/// Address of the WB32 ROM bootloader's vector table.
///
/// The first word there is the bootloader's initial `MSP`, the second its reset
/// vector. This is QMK's `WB32_BOOTLOADER_ADDRESS`, the default for both WB32
/// families in `platforms/chibios/mcu_selection.mk` (`?= 0x1FFFE000`, the
/// WB32FQ95xx branch), passed to `wb32_dfu.c` via `-DWB32_BOOTLOADER_ADDRESS`.
pub const WB32_BOOTLOADER_ADDRESS: u32 = 0x1FFF_E000;

/// Sentinel the reset path looks for. QMK's `BOOTLOADER_MAGIC`.
const BOOTLOADER_MAGIC: u32 = 0xDEAD_BEEF;

/// The reserved magic cell: the last word of physical SRAM
/// (`0x2000_0000 + 28K - 4`). See the [module docs](self) for why it survives a
/// reset and is never overwritten by a section or the stack. Kept in lockstep
/// with the `RAM` length in `firmware/memory.x`.
const MAGIC_ADDR: *mut u32 = 0x2000_6FFC as *mut u32;

/// Arm the magic word and reset; the reset path then enters the `wb32-dfu`
/// bootloader. Never returns.
///
/// Port of QMK's `bootloader_jump` (`wb32_dfu.c`): write the magic, then
/// `NVIC_SystemReset()`. The `dsb` between the two guarantees the store has
/// reached SRAM before the core resets.
pub fn bootloader_jump() -> ! {
    // SAFETY: `MAGIC_ADDR` is a fixed, always-valid SRAM word reserved out of
    // every linker section and out of the stack (see module docs), so this write
    // races with nothing.
    unsafe {
        core::ptr::write_volatile(MAGIC_ADDR, BOOTLOADER_MAGIC);
    }
    cortex_m::asm::dsb();
    SCB::sys_reset();
}

/// Reset the MCU normally — no magic, so the reset path boots the firmware as
/// usual. Backs the kcp `SYSTEM.REBOOT` command. Never returns.
pub fn reboot() -> ! {
    SCB::sys_reset();
}

/// Run before `main`, before RAM is initialised: if [`bootloader_jump`] armed
/// the magic, hand control to the `wb32-dfu` ROM bootloader instead of starting
/// the firmware.
///
/// Port of QMK's `enter_bootloader_mode_if_requested`. cortex-m-rt's `Reset`
/// calls `__pre_init` (this) *before* it zeroes `.bss` and copies `.data`, so
/// the magic cell — which belongs to no such section anyway — is read with its
/// pre-reset value intact.
///
/// [`cortex_m::asm::bootload`] is the purpose-built, non-deprecated equivalent
/// of the C's tail: it reads `MSP` from `*(WB32_BOOTLOADER_ADDRESS)` and the
/// reset vector from `*(WB32_BOOTLOADER_ADDRESS + 4)`, clears `CONTROL.SPSEL`
/// (the `__set_CONTROL(0)` selecting the main stack), sets `MSP`, and branches to
/// the reset vector with the Thumb bit set — all in one asm sequence, so no
/// stack access can occur between switching `MSP` and the jump. The explicit
/// [`cortex_m::interrupt::enable`] mirrors the C's `__enable_irq()`; it is a
/// belt-and-suspenders `cpsie i` since `PRIMASK` is already clear and no NVIC
/// source is enabled this early after reset.
///
/// When the magic is unset this returns immediately and the normal boot proceeds
/// untouched — a single fixed-address load and compare, zero effect on a normal
/// power-on.
///
/// # Safety
///
/// Runs before RAM init. Per cortex-m-rt's `#[pre_init]` contract it must not
/// touch any Rust `static` (their backing memory is uninitialised this early) nor
/// take a reference that could be promoted to one. It does neither: only
/// `read_volatile`/`write_volatile` through the fixed `MAGIC_ADDR` immediate and
/// register/asm intrinsics, all on memory outside the `.bss`/`.data` regions.
#[cortex_m_rt::pre_init]
unsafe fn enter_bootloader_mode_if_requested() {
    if core::ptr::read_volatile(MAGIC_ADDR) == BOOTLOADER_MAGIC {
        // Clear it first so a spurious reset after this point boots normally
        // rather than looping back into the bootloader (matches `*check = 0`).
        core::ptr::write_volatile(MAGIC_ADDR, 0);
        cortex_m::asm::dsb();
        cortex_m::interrupt::enable(); // __enable_irq()
        // CONTROL.SPSEL = 0 + MSP <- *(BOOT) + jump to *(BOOT+4). Never returns.
        cortex_m::asm::bootload(WB32_BOOTLOADER_ADDRESS as *const u32);
    }
    // Magic unset: fall through, return to `Reset`, boot the firmware normally.
}
