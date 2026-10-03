/* ARENA_SHARED.H */

pub const OS_ARENA_MAIN: u32 = 0;
pub const OS_ARENA_MAIN_SUBPRIV: u32 = 1;
pub const OS_ARENA_MAINEX: u32 = 2;
pub const OS_ARENA_ITCM: u32 = 3;
pub const OS_ARENA_DTCM: u32 = 4;
pub use crate::sys::OS_ARENA_SYS_AREA_SHARED;
pub const OS_ARENA_WRAM_SHARED_ARM9: u32 = 6;
pub const OS_ARENA_WRAM_SHARED_ARM7: u32 = 7;
pub const OS_ARENA_WRAM_SUBPRIV: u32 = 8;
pub const OS_ARENA_MAX: usize = 9;

/* CONSTS.H */

pub const OSi_CONSOLE_NOT_DETECT: u32 = 0xFFFFFFFF;

pub const OS_CONSOLE_NITRO: u32 = 0x80000000;
pub const OS_CONSOLE_DEV_CARD: u32 = 0x02000000;
pub const OS_CONSOLE_SIZE_MASK: u32 = 0x00000003;
pub const OS_CONSOLE_SIZE_4MB: u32 = 0x00000001;

/* ARM9/IO_REG.H */

pub const REG_OS_IE_GF_SHIFT: usize = 21;

/* MMAP.H */

/// Beyond this is private memory reserved for the hardware debugger when the game is
/// built and run on the Debug Console with 8 MiB Main RAM.
pub const HW_MAIN_MEM_DEBUGGER_OFFSET: u32 = 0x00700000;

