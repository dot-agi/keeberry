/* The last eighteen 256-byte flash pages (0x0803EE00..0x08040000, 4608 bytes) are
   reserved for persistence (the full-config blob, CONFIG_REGION) and MUST NOT
   hold code or data the linker places. FLASH is shortened to end at
   CONFIG_REGION's start so the toolchain can never emit anything into that
   window; the reserved tail is written only by the FMC driver, which
   additionally hard-asserts the bound (see firmware/src/flash.rs,
   `CONFIG_REGION`/`CONFIG_PAGES`). This `LENGTH` is CONFIG_REGION.start -
   ORIGIN(FLASH); keep the two in lockstep (flash.rs derives the same split from
   CONFIG_PAGES = 18). 0x3EE00 = 256K - 4.5K. The WB32FQ95 die has 256 KiB of main
   flash (confirmed on silicon by a full DFU read + aliasing test; the `xB`
   datasheet's 128 KiB figure is wrong for this part). */
/* The top eight bytes of the 28 KiB SRAM are carved out of the linker's RAM
   region so the software-DFU magic word survives a reset. The magic lives in the
   last physical word, 0x20006FFC (= 0x20000000 + 28K - 4, QMK's __ram0_end__ - 4;
   see firmware/src/boot.rs). With RAM shortened to 28K - 8, cortex-m-rt sets
   _stack_start = ORIGIN(RAM) + LENGTH(RAM) = 0x20006FF8, which is 8-byte aligned
   (AAPCS + cortex-m-rt's ASSERT(_stack_start % 8 == 0) both require it); the
   full-descending main stack's first push lands at 0x20006FF4, and .data/.bss/
   .uninit are all confined to RAM, so nothing the toolchain places — nor the
   stack — ever touches 0x20006FFC. The word at 0x20006FF8 is alignment padding.
   Eight bytes, not four, only to keep that 8-byte alignment. */
MEMORY
{
  FLASH : ORIGIN = 0x08000000, LENGTH = 0x3EE00
  RAM   : ORIGIN = 0x20000000, LENGTH = 28K - 8
}
