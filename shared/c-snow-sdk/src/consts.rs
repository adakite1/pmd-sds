pub const ARM7_FIFO_HS_MAINPROC_MUST_RESPOND_WITHIN: u32 = 1000; // CPU cycles,
// with each `swi 3` spinloop taking ~4 clock cycles, translating into
// approximately that value divided by 4.
// ARM9 must respond to the message within this window, and if by the end of
// this window the ARM9 has not yet echoed the next number (whether it
// echoes early in the window does not matter since we always spin
// in full), we start over from 8 again (more precisely 7 is the next message
// due to the loop's i--).
pub const ARM7_FIFO_HS_SUCCESS_AFTER: u32 = 8; // messages sent and seen echoed.
// Only after all messages 7..=0 get echoed by ARM9 do we exit out of the handshake
// loop to continue forward.
pub const ARM9_FIFO_HS_ACCEPT_SUCCESS_AFTER: u32 = 4; // changes in the value.
// Only after we see 4 consecutive changes in the value sent from ARM7 do we then
// start accepting 0 (success) as the next number after that.
pub const ARM9_FIFO_HS_SPINWAIT_FOR_SUBPROC_NEXT_NUMBER_BEFORE_RESET: u32 = 1000; // polls + 1 since the end condition is overflow.
// This wait is the same number but doesn't imply a connection to ARM7_FIFO_HS_MAINPROC_MUST_RESPOND_WITHIN.
// It is a simple spinning check on SYNC, until ARM7 responds with the next
// number to echo back. If this count is lapsed, we start over from 0 (more precisely 1
// is the next count i due to the loop's i++, which means the lapsed round is counted and
// one less round of success is necessary for the ARM9 going forwards until it accepts a
// success).
pub const DMA_CHANNELS_COUNT: usize = REG_OS_IE_D3_SHIFT - REG_OS_IE_D0_SHIFT + 1;
pub use crate::sys::dtcm_sys::DTCM_TOP;
pub use crate::sys::dtcm_sys::IRQ_FUTEX_BITS;
pub use crate::sys::dtcm_sys::IRQ_HANDLER_POINTER;
pub const OSi_IRQ_CALLBACK_MAX: usize = 8;
pub use crate::sys::SYS_AREA_START;
pub use crate::sys::SYS_AREA_SYSTEM_WORK;
pub const TIMERS_COUNT: usize = REG_OS_IE_T3_SHIFT - REG_OS_IE_T0_SHIFT + 1;

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

/* ARMARCH.H */

pub const HW_PSR_IRQ_DISABLE: u32 = 0x80;

/* CONSTS.H */

pub const OSi_CONSOLE_NOT_DETECT: u32 = 0xFFFFFFFF;

pub const OS_CONSOLE_NITRO: u32 = 0x80000000;
pub const OS_CONSOLE_DEV_CARD: u32 = 0x02000000;
pub const OS_CONSOLE_SIZE_MASK: u32 = 0x00000003;
pub const OS_CONSOLE_SIZE_4MB: u32 = 0x00000001;

/* ARM9/IO_REG.H */

pub const REG_OS_IE_GF_SHIFT: usize = 21;

pub const REG_OS_IE_D3_SHIFT: usize = 11;

pub const REG_OS_IE_D0_SHIFT: usize = 8;

pub const REG_OS_IE_T3_SHIFT: usize = 6;

pub const REG_OS_IE_T0_SHIFT: usize = 3;

/* MMAP.H */

/// Beyond this is private memory reserved for the hardware debugger when the game is
/// built and run on the Debug Console with 8 MiB Main RAM.
pub const HW_MAIN_MEM_DEBUGGER_OFFSET: u32 = 0x00700000;

