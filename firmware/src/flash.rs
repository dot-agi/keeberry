// SPDX-License-Identifier: GPL-2.0-or-later
//! WB32FQ95 embedded-flash (FMC) erase/program driver.
//!
//! This is a faithful port of the WB32 vendor embedded-flash low-level driver
//! (ChibiOS-Contrib `os/hal/ports/WB32/WB32FQ95xx/hal_efl_lld.c`, functions
//! `wb32_flash_op_exec`, `wb32_flash_erase_page`, `wb32_flash_clear_page_latch`
//! and `wb32_flash_program_page`, plus the per-page body of `efl_lld_program`).
//! Register addresses and bit fields are taken from the CMSIS device header
//! `os/common/ext/CMSIS/WB32/WB32FQ95xx/wb32fq95xx.h`. Every magic value below
//! cites the source line it derives from.
//!
//! # No PAC peripheral for the FMC
//!
//! The `pac` does not model the FMC (flash memory controller), so this
//! module talks to it through raw volatile MMIO at [`FMC_BASE`] using the
//! register map from `wb32fq95xx.h:378-389`. RCC's program-clock-enable register
//! (`RCC.PCLKENR`) is likewise written by raw MMIO so the driver is self
//! contained and needs no borrow of the PAC `Peripherals`.
//!
//! # Why two machine-code blobs are executed verbatim
//!
//! The vendor performs the actual erase/program **trigger and busy-wait** from a
//! short routine copied into RAM ([`FLASH_OP_RAM_CODE`], `hal_efl_lld.c:41,77`),
//! because the flash array cannot be fetched from while it is being written, and
//! it runs a larger opaque **pre-operation** routine ([`PRE_OP_CODE`],
//! `hal_efl_lld.c:43-44`) that touches power/clock trims and the chip's
//! info-flash (`0x1FFFF000`). Re-deriving `PRE_OP_CODE` would mean guessing at
//! undocumented hardware, so both blobs are carried **byte-for-byte** from the
//! vendor LLD and invoked exactly as it does — the trigger from RAM, the
//! pre-operation from flash. This keeps the sequence provably identical to the
//! vendor's without inventing any register semantics. The only Rust-level logic
//! is the [`CONFIG_REGION`] address guard and the documented CON/KEY/ADDR/BUF/
//! STAT/PCLKENR register writes.
//!
//! # Safety: the reserved-region guard
//!
//! [`erase_page`] and [`program_page`] hard-`assert!` that their target is a page
//! inside the reserved [`CONFIG_REGION`] — the reserved tail pages of flash —
//! *before* touching any FMC register. The firmware image lives at the start of flash and
//! is far smaller than the 256 KiB window, so it can never overlap the reserved
//! tail; `memory.x` additionally shortens the linker's FLASH window to end at
//! [`CONFIG_REGION`]'s start so no code is ever placed in it. The address register
//! is the only thing that selects which page an op affects, and it is written only
//! from the guarded, page-aligned value, so a hardware op can physically only
//! reach the reserved region.

use core::ops::Range;

// === Flash geometry ========================================================

/// Base address of the memory-mapped embedded flash (`FLASH_BASE`,
/// wb32fq95xx.h:951; also `memory.x` `ORIGIN(FLASH)`).
pub const FLASH_BASE: u32 = 0x0800_0000;

/// Total embedded-flash size: 256 KiB. The WB32FQ95 die has 256 KiB of main flash
/// (confirmed on silicon by a full DFU read + aliasing test — the upper 128 KiB is
/// distinct erased flash, not a mirror of the lower bank; the `xB` datasheet's
/// 128 KiB figure is wrong for this part). This is the end of the flash address
/// space; the linker `LENGTH(FLASH)` is this less the reserved [`CONFIG_REGION`] tail.
pub const FLASH_SIZE: u32 = 256 * 1024;

/// Flash page / sector size, in bytes. This is both the erase granularity
/// (`WB32_FLASH_SECTOR_SIZE`, wb32_registry.h:74) and the program granularity
/// (`WB32_FLASH_PAGE_SIZE`, hal_efl_lld.c:38). One page is also the size of the
/// FMC page buffer [`FMC_BUF`] (64 words, wb32fq95xx.h:388).
pub const PAGE_SIZE: usize = 256;

/// Number of 32-bit words in one flash page ([`PAGE_SIZE`] / 4 = 64), i.e. the
/// length of the FMC page buffer programmed per op (hal_efl_lld.c:339-341).
const PAGE_WORDS: usize = PAGE_SIZE / 4;

/// Number of 256-byte pages reserved for persistence at the tail of flash.
///
/// Sized to hold the worst-case full-config blob (see [`crate::config`], which
/// hard-asserts the fit: every keymap layer + NKRO + RGB + SOCD/override/
/// tap-dance/combo/macro table at full capacity) with one spare page of headroom.
/// Eighteen pages = 4608 bytes; the sixteen-layer blob's max (4116 B, ~4.0 KiB) is
/// under that. `memory.x`'s `LENGTH(FLASH)` must equal [`CONFIG_REGION`]`.start -
/// FLASH_BASE` — both are this same split (256 KiB − `CONFIG_PAGES`·256), kept in
/// lockstep by hand.
pub const CONFIG_PAGES: u32 = 18;

/// Reserved configuration region: the **last [`CONFIG_PAGES`] flash pages**
/// (4608 bytes), `0x0803_EE00 ..= 0x0803_FFFF`, the very end of the 256 KiB
/// flash.
///
/// It holds the versioned, CRC-protected full-config blob (see [`crate::config`])
/// and forms the reserved flash tail that `memory.x` excludes from the linker
/// FLASH window so nothing else is ever linked here. Derived from
/// [`CONFIG_PAGES`] so `start` stays page-aligned and `end` stays the flash end.
pub const CONFIG_REGION: Range<u32> =
    (FLASH_BASE + FLASH_SIZE - CONFIG_PAGES * PAGE_SIZE as u32)..(FLASH_BASE + FLASH_SIZE);

// Compile-time proof the reserved CONFIG region is exactly the flash tail and
// spans a whole number of pages, so page-aligned guarding covers it with no gaps
// and reaches nothing else.
const _: () = assert!(CONFIG_REGION.end == FLASH_BASE + FLASH_SIZE);
const _: () = assert!((CONFIG_REGION.end - CONFIG_REGION.start) % PAGE_SIZE as u32 == 0);
const _: () = assert!((CONFIG_REGION.start - FLASH_BASE) % PAGE_SIZE as u32 == 0);

// === FMC register block (raw MMIO) =========================================
// FMC_BASE = AHBPERIPH_BASE(0x4001_0000) + 0x7800 (wb32fq95xx.h:967). Register
// offsets from the `FMC_TypeDef` layout (wb32fq95xx.h:378-389).

/// FMC peripheral base address (`FMC_BASE`, wb32fq95xx.h:967).
pub const FMC_BASE: u32 = 0x4001_7800;

/// FMC control register `CON` (offset 0x000, wb32fq95xx.h:380).
const FMC_CON: *mut u32 = (FMC_BASE + 0x000) as *mut u32;
/// FMC status register `STAT` (offset 0x008, wb32fq95xx.h:382).
const FMC_STAT: *mut u32 = (FMC_BASE + 0x008) as *mut u32;
/// FMC key register `KEY` (offset 0x00C, wb32fq95xx.h:383).
const FMC_KEY: *mut u32 = (FMC_BASE + 0x00C) as *mut u32;
/// FMC address register `ADDR` (offset 0x010, wb32fq95xx.h:384).
const FMC_ADDR: *mut u32 = (FMC_BASE + 0x010) as *mut u32;
/// FMC page buffer `BUF[64]` (offset 0x100, wb32fq95xx.h:388).
const FMC_BUF: *mut u32 = (FMC_BASE + 0x100) as *mut u32;

/// RCC program-clock-enable register `RCC.PCLKENR`
/// (RCC_BASE 0x4001_0C00, wb32fq95xx.h:959 + offset 0x060, wb32fq95xx.h:818).
const RCC_PCLKENR: *mut u32 = 0x4001_0C60 as *mut u32;

// === FMC bit fields and op codes ===========================================

/// `FMC_CON.OP[4:0]` field mask — selects the flash operation
/// (`FMC_CON_OP_Msk = 0x1FU`, wb32fq95xx.h:3133).
const FMC_CON_OP_MSK: u32 = 0x1F;
/// `FMC_CON.WR` (bit 7) — the operation "go"/busy bit
/// (`FMC_CON_WR = 0x1U << 7`, wb32fq95xx.h:3136). Set to start an op; polled
/// until it self-clears to detect completion (cf. `efl_lld_query_erase`:451).
const FMC_CON_WR: u32 = 0x1 << 7;
/// `FMC_STAT.ERR` (bit 2) — any-error flag checked after an op
/// (`FMC_STAT_ERR = 0x1U << 2`, wb32fq95xx.h:3154).
const FMC_STAT_ERR: u32 = 0x1 << 2;

/// FMC op code: clear page latch (hal_efl_lld.c:140, `wb32_flash_op_exec(..,0x04)`).
const OP_CLEAR_LATCH: u32 = 0x04;
/// FMC op code: erase page (hal_efl_lld.c:123, `wb32_flash_op_exec(..,0x08)`).
const OP_ERASE_PAGE: u32 = 0x08;
/// FMC op code: program page (hal_efl_lld.c:104, `wb32_flash_op_exec(..,0x0C)`).
const OP_PROGRAM_PAGE: u32 = 0x0C;

/// `FMC.CON` setup value OR-ed with the op code (hal_efl_lld.c:81).
///
/// Per the vendor comment, this has `WREN`(bit 6)=1, `WR`(bit 7)=0 and
/// `SETHLDCNT`[14:8]=0x0D (`FMC_CON_WREN`/`FMC_CON_WR`/`FMC_CON_SETHLDCNT_Msk`,
/// wb32fq95xx.h:3135-3138). The remaining upper bits (`0x7F5F_0000`) are vendor
/// timing/configuration not broken out in the CMSIS header; they are carried
/// **verbatim** from the LLD rather than reconstructed.
const CON_OP_SETUP: u32 = 0x7F5F_0D40;
/// `FMC.CON` teardown value after an op (hal_efl_lld.c:88): clears `WREN` and
/// `OP[4:0]` while leaving the vendor `0x005F_0000` field in place (verbatim).
const CON_TEARDOWN: u32 = 0x005F_0000;

/// First `FMC.KEY` unlock word (hal_efl_lld.c:82).
const FMC_KEY1: u32 = 0x5188_DA08;
/// Second `FMC.KEY` unlock word (hal_efl_lld.c:83).
const FMC_KEY2: u32 = 0x1258_6590;

/// Value the RAM trigger stores into `FMC.CON` to launch the op
/// (hal_efl_lld.c:84, first argument to the RAM routine), i.e. `0x0080_0080`.
/// Bit 7 is [`FMC_CON_WR`] (the "go" bit); bit 23 (`0x0080_0000`) is undocumented
/// in the CMSIS header and is carried **verbatim** from the LLD. After the store
/// the routine polls `WR` until it clears.
const OP_TRIGGER: u32 = FMC_CON_WR | 0x0080_0000;

/// `RCC.PCLKENR` value enabling the FMC program clock around an op
/// (hal_efl_lld.c:80, `RCC->PCLKENR = 0x01`).
const PCLKENR_FMC_ON: u32 = 0x01;
/// `RCC.PCLKENR` value restored after an op (hal_efl_lld.c:86, `= 0x00`).
const PCLKENR_FMC_OFF: u32 = 0x00;

// === Vendor machine-code blobs (carried verbatim) ==========================

/// Flash-operation trigger + busy-wait, executed **from RAM**
/// (`FLASH_OP_RAM_CODE`, hal_efl_lld.c:41). Called as `fn(val: u32, base: u32)`;
/// disassembly (Thumb, little-endian halfword order):
///
/// ```text
///   STR  r0,[r1]        ; *FMC.CON = val  (val = OP_TRIGGER, sets WR=1)   6008
///   NOP                                                                    bf00
/// loop:
///   LDR  r0,[r1]        ; r0 = FMC.CON                                     6808
///   LSLS r0,r0,#24      ; move bit 7 (WR) into N                           0600
///   BMI  loop           ; spin while WR set                               d4fc
///   BX   lr                                                                4770
/// ```
///
/// It must run from RAM because the store starts the flash operation, after
/// which the flash array is busy and instruction fetch from it would stall/fault
/// until completion. The constants are copied into a stack buffer at call time
/// (see [`run_op_trigger`]).
const FLASH_OP_RAM_CODE: [u32; 3] = [0xbf00_6008, 0x0600_6808, 0x4770_d4fc];

/// Vendor pre-operation routine (`pre_op_code`, hal_efl_lld.c:43), run before
/// each erase/program with interrupts disabled (hal_efl_lld.c:102,121). It is an
/// opaque blob that prepares power/clock trims from the chip info-flash (the
/// trailing constants `0x4001_0000`, `0x4001_0438`, `0x4001_0C20`, `0x4000_B804`
/// and `0x1FFF_F000` are its PC-relative literal pool). It is executed from
/// flash, exactly as the vendor does (it is `static const` there too): at this
/// point no flash operation is in progress, so instruction fetch is valid. It is
/// carried byte-for-byte rather than re-derived because its behaviour is not
/// documented.
static PRE_OP_CODE: [u32; 57] = [
    0x4ff0_e92d, 0x2103_4832, 0x210c_6281, 0xf8df_62c1, 0x2100_c0c4, 0x1000_f8cc,
    0xf44f_4608, 0x1c40_767a, 0xdbfc_42b0, 0xf8cc_2201, 0x2000_2000, 0x42b0_1c40,
    0x4829_dbfc, 0xf043_6803, 0x6003_0380, 0x302c_4826, 0xf443_6803, 0x6003_6320,
    0x4610_4691, 0x323c_4a22, 0x468a_6010, 0x4921_4608, 0x4821_6008, 0x0340_f8d0,
    0x2500_4f1e, 0x5107_f3c0, 0x3bff_f04f, 0x2200_1f3f, 0x4610_465c, 0xea5f_683b,
    0xd106_78c0, 0xd101_42a3, 0xe000_2401, 0x4422_2400, 0x1c40_461c, 0xdbf1_2814,
    0xd91b_2a02, 0xd901_2910, 0xe000_3910, 0x480d_2100, 0x6802_1f00, 0x627f_f022,
    0x5201_ea42, 0xf8cc_6002, 0x2000_a000, 0x42b0_1c40, 0xf8cc_dbfc, 0x2000_9000,
    0x42b0_1c40, 0x1c6d_dbfc, 0xdbd0_2d05, 0x8ff0_e8bd, 0x4001_0000, 0x4001_0438,
    0x4001_0c20, 0x4000_b804, 0x1fff_f000,
];

// === Low-level op execution ================================================

/// Execute [`PRE_OP_CODE`] from flash (the vendor `PRE_OP()`, hal_efl_lld.c:44).
///
/// # Safety
/// Must be called with interrupts disabled and no flash op in progress, exactly
/// as the vendor invokes it (hal_efl_lld.c:102,121).
#[inline(never)]
unsafe fn pre_op() {
    // Address of the contiguous blob, with the Thumb bit (bit 0) set so the
    // `BX`/`BLX` into it stays in Thumb state.
    let entry = (PRE_OP_CODE.as_ptr() as usize) | 1;
    let f: extern "C" fn() = core::mem::transmute(entry);
    f();
}

/// Copy [`FLASH_OP_RAM_CODE`] to the stack and run it to trigger one flash op
/// and busy-wait for completion (the inner RAM call of `wb32_flash_op_exec`,
/// hal_efl_lld.c:84).
///
/// # Safety
/// `FMC.CON`/`FMC.KEY` must already be set up for the op (caller does this), and
/// interrupts must be disabled.
#[inline(never)]
unsafe fn run_op_trigger() {
    // Stack-resident copy so the trigger + busy-wait execute from RAM (see
    // [`FLASH_OP_RAM_CODE`]); SRAM is executable on this core (no MPU). Written
    // volatile so the compiler can't elide the buffer, then DSB+ISB so the CPU
    // fetches the freshly written instructions (ARM self-modifying-code rule).
    let mut ram_code = [0u32; 3];
    for (i, &w) in FLASH_OP_RAM_CODE.iter().enumerate() {
        core::ptr::write_volatile(&mut ram_code[i], w);
    }
    cortex_m::asm::dsb();
    cortex_m::asm::isb();

    let entry = (ram_code.as_ptr() as usize) | 1;
    let f: extern "C" fn(u32, u32) = core::mem::transmute(entry);
    f(OP_TRIGGER, FMC_BASE);
}

/// Run one FMC operation (`wb32_flash_op_exec`, hal_efl_lld.c:76-94).
///
/// Returns `true` if `FMC.STAT.ERR` is set afterwards (operation failed).
///
/// # Safety
/// Interrupts must be disabled by the caller for erase/program ops (so the
/// preceding [`pre_op`] and `FMC.ADDR` setup are atomic with the trigger).
unsafe fn op_exec(op: u32) -> bool {
    core::ptr::write_volatile(RCC_PCLKENR, PCLKENR_FMC_ON); // :80
    core::ptr::write_volatile(FMC_CON, CON_OP_SETUP | (op & FMC_CON_OP_MSK)); // :81
    core::ptr::write_volatile(FMC_KEY, FMC_KEY1); // :82
    core::ptr::write_volatile(FMC_KEY, FMC_KEY2); // :83
    run_op_trigger(); // :84 — STR OP_TRIGGER -> CON, poll WR (from RAM)
    core::ptr::write_volatile(RCC_PCLKENR, PCLKENR_FMC_OFF); // :86
    core::ptr::write_volatile(FMC_CON, CON_TEARDOWN); // :88
    (core::ptr::read_volatile(FMC_STAT) & FMC_STAT_ERR) != 0 // :90
}

/// Erase one page (`wb32_flash_erase_page`, hal_efl_lld.c:115-132): pre-op, set
/// `FMC.ADDR`, run op `0x08`. Returns `true` on error.
///
/// # Safety
/// `page_addr` must be a valid, page-aligned flash address; callers guard it.
unsafe fn flash_erase_page(page_addr: u32) -> bool {
    // The closure does not inherit this `unsafe fn`'s context, hence the block.
    critical_section::with(|_| unsafe {
        pre_op(); // :121
        core::ptr::write_volatile(FMC_ADDR, page_addr); // :122
        op_exec(OP_ERASE_PAGE) // :123
    })
}

/// Clear the page latch (`wb32_flash_clear_page_latch`, hal_efl_lld.c:134-149):
/// run op `0x04` (no pre-op, no address). Returns `true` on error.
unsafe fn flash_clear_page_latch() -> bool {
    critical_section::with(|_| unsafe { op_exec(OP_CLEAR_LATCH) }) // :140
}

/// Program the loaded page buffer (`wb32_flash_program_page`, hal_efl_lld.c:96-113):
/// pre-op, set `FMC.ADDR`, run op `0x0C`. Returns `true` on error.
///
/// # Safety
/// `FMC.BUF` must be filled first; `page_addr` must be page-aligned and guarded.
unsafe fn flash_program_page(page_addr: u32) -> bool {
    critical_section::with(|_| unsafe {
        pre_op(); // :102
        core::ptr::write_volatile(FMC_ADDR, page_addr); // :103
        op_exec(OP_PROGRAM_PAGE) // :104
    })
}

// === Address guard =========================================================

/// `true` iff `[addr, addr+PAGE_SIZE)` is a whole page inside `[start, end)`.
///
/// Because each region's `start` is itself page-aligned, requiring `addr` to be
/// an exact multiple of [`PAGE_SIZE`] above the start (and the page to fit below
/// the end) restricts the match to whole reserved pages and nothing else.
const fn page_in(addr: u32, start: u32, end: u32) -> bool {
    addr >= start && (addr - start) % PAGE_SIZE as u32 == 0 && addr <= end - PAGE_SIZE as u32
}

/// `true` iff `addr` is a whole page inside the reserved [`CONFIG_REGION`] — and
/// nothing else.
const fn is_writable_page(addr: u32) -> bool {
    page_in(addr, CONFIG_REGION.start, CONFIG_REGION.end)
}

/// Panic with a clear message if `addr` is not a guarded, page-aligned page in
/// the reserved [`CONFIG_REGION`]. Called at the top of every public write entry
/// point, before any FMC register is touched, so an out-of-region address can
/// never reach the hardware.
fn assert_writable(addr: u32) {
    assert!(
        is_writable_page(addr),
        "flash: refusing to write outside the reserved CONFIG region"
    );
}

// === Public API ============================================================

/// Erase one 256-byte flash page at `addr`.
///
/// `addr` must be a page in [`CONFIG_REGION`]; this is hard-asserted before any
/// hardware access. After erase the page reads back as `0x00`
/// (`efl_lld_verify_erase` treats `0x00` as the erased state, hal_efl_lld.c:514).
/// Returns `Err(())` if `FMC.STAT.ERR` is set.
///
/// # Safety
/// Writes to flash; the caller must ensure no concurrent access to the page and
/// accept that interrupts are briefly disabled during the op.
pub unsafe fn erase_page(addr: u32) -> Result<(), ()> {
    assert_writable(addr);
    if flash_erase_page(addr) {
        Err(())
    } else {
        Ok(())
    }
}

/// Program one already-erased 256-byte page at `addr` with `data`, then verify.
///
/// Mirrors the per-page body of `efl_lld_program` (hal_efl_lld.c:333-359) for a
/// caller-erased page: clear the page latch, fill `FMC.BUF[0..64]` from `data`
/// (little-endian words, hal_efl_lld.c:339-341), program op `0x0C`, then read
/// back and compare every word (the vendor's `memcmp`, hal_efl_lld.c:350).
///
/// `addr` must be a page in [`CONFIG_REGION`] (hard-asserted). The caller is
/// expected to have [`erase_page`]d it first. Returns `Err(())` on a hardware
/// error or a read-back mismatch.
///
/// # Safety
/// Writes to flash; same constraints as [`erase_page`].
pub unsafe fn program_page(addr: u32, data: &[u8; PAGE_SIZE]) -> Result<(), ()> {
    assert_writable(addr);

    // Clear page latch (op 0x04) before loading the buffer.
    if flash_clear_page_latch() {
        return Err(());
    }

    // Load the 64-word page buffer from `data` (little-endian), :339-341.
    for i in 0..PAGE_WORDS {
        let w = u32::from_le_bytes([
            data[i * 4],
            data[i * 4 + 1],
            data[i * 4 + 2],
            data[i * 4 + 3],
        ]);
        core::ptr::write_volatile(FMC_BUF.add(i), w);
    }

    // Program the page (op 0x0C).
    if flash_program_page(addr) {
        return Err(());
    }

    // Verify: compare every programmed word against `data` (:350).
    let page = addr as *const u32;
    for i in 0..PAGE_WORDS {
        let want = u32::from_le_bytes([
            data[i * 4],
            data[i * 4 + 1],
            data[i * 4 + 2],
            data[i * 4 + 3],
        ]);
        if core::ptr::read_volatile(page.add(i)) != want {
            return Err(());
        }
    }
    Ok(())
}

/// Read `buf.len()` bytes of memory-mapped flash starting at `addr` into `buf`.
///
/// Flash is memory-mapped and readable everywhere, so this is a plain volatile
/// copy and needs no region guard. `addr + buf.len()` must stay within the flash
/// window (`FLASH_BASE .. FLASH_BASE + FLASH_SIZE`).
pub fn read(addr: u32, buf: &mut [u8]) {
    debug_assert!(
        addr >= FLASH_BASE && (addr as u64 + buf.len() as u64) <= (FLASH_BASE + FLASH_SIZE) as u64,
        "flash: read out of bounds"
    );
    let src = addr as *const u8;
    for (i, b) in buf.iter_mut().enumerate() {
        // SAFETY: bounded by the debug assert above; flash is always readable.
        *b = unsafe { core::ptr::read_volatile(src.add(i)) };
    }
}
