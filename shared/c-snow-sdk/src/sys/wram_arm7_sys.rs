//! Top 0x40 bytes of ARM7 WRAM reserved for SDK and BIOS uses. The arm9 version is in DTCM.
//!
//! Most constants here are ordered top-down by address within that 0x40 bytes slice.

pub const WRAM_TOP: u32 = 0x03810000; //NOTE: This should be the same value as the __wram_top value defined in the arm7 linker script, make sure to keep it up to date

pub const IRQ_HANDLER_POINTER_ARM7: u32 = WRAM_TOP - 4;

/// See [`crate::sys::dtcm_sys::IRQ_FUTEX_BITS_ARM9`].
pub const IRQ_FUTEX_BITS_ARM7: u32 = WRAM_TOP - 8;

