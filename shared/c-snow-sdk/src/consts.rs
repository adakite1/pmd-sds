/* CONSTS.H */

pub const OSi_CONSOLE_NOT_DETECT: u32 = 0xFFFFFFFF;

pub const OS_CONSOLE_NITRO: u32 = 0x80000000;
pub const OS_CONSOLE_DEV_CARD: u32 = 0x02000000;
pub const OS_CONSOLE_SIZE_MASK: u32 = 0x00000003;
pub const OS_CONSOLE_SIZE_4MB: u32 = 0x00000001;

/* MMAP.H */

/// Beyond this is private memory reserved for the hardware debugger when the game is
/// built and run on the Debug Console with 8 MiB Main RAM.
pub const HW_MAIN_MEM_DEBUGGER_OFFSET: u32 = 0x00700000;

