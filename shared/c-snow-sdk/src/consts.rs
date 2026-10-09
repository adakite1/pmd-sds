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
pub const DMA_CHANNELS_COUNT: usize = REG_IE_DMA_3_FINISHED_SHIFT - REG_IE_DMA_0_FINISHED_SHIFT + 1;
pub use crate::sys::dtcm_sys::DTCM_TOP;
#[cfg(armv5te)]
pub use crate::sys::dtcm_sys::IRQ_FUTEX_BITS_ARM9 as IRQ_FUTEX_BITS;
#[cfg(armv5te)]
pub use crate::sys::dtcm_sys::IRQ_HANDLER_POINTER_ARM9 as IRQ_HANDLER_POINTER;
#[cfg(armv4t)]
pub use crate::sys::wram_arm7_sys::IRQ_FUTEX_BITS_ARM7 as IRQ_FUTEX_BITS;
#[cfg(armv4t)]
pub use crate::sys::wram_arm7_sys::IRQ_HANDLER_POINTER_ARM7 as IRQ_HANDLER_POINTER;
pub const OSi_IRQ_CALLBACK_MAX: usize = 8;
pub const PXI_MAX_FIFO_MESSAGE_TAG: usize = 32;

pub const  REG_FIFO_CNT_ENABLE_SHIFT: usize = 15;
pub const  REG_FIFO_CNT_ERROR_SHIFT: usize = 14;
pub const  REG_FIFO_CNT_IRQ_UPON_RECV_NON_EMPTY_SHIFT: usize = 10;
pub const _REG_FIFO_CNT_RO_RECV_FULL_SHIFT: usize = 9;
pub const _REG_FIFO_CNT_RO_RECV_EMPTY_SHIFT: usize = 8;
pub const  REG_FIFO_CNT_CLEAR_SEND_QUEUE_SHIFT: usize = 3;
pub const _REG_FIFO_CNT_IRQ_UPON_SEND_EMPTY_SHIFT: usize = 2;
pub const _REG_FIFO_CNT_RO_SEND_FULL_SHIFT: usize = 1;
pub const _REG_FIFO_CNT_RO_SEND_EMPTY_SHIFT: usize = 0;

/** 0x8000 */ pub const  REG_FIFO_CNT_ENABLE_MASK: u16 = 1 << REG_FIFO_CNT_ENABLE_SHIFT;
/** 0x4000 */ pub const  REG_FIFO_CNT_ERROR_MASK: u16 = 1 << REG_FIFO_CNT_ERROR_SHIFT;
/** 0x0400 */ pub const  REG_FIFO_CNT_IRQ_UPON_RECV_NON_EMPTY_MASK: u16 = 1 << REG_FIFO_CNT_IRQ_UPON_RECV_NON_EMPTY_SHIFT;
/** 0x0200 */ pub const _REG_FIFO_CNT_RO_RECV_FULL_MASK: u16 = 1 << _REG_FIFO_CNT_RO_RECV_FULL_SHIFT;
/** 0x0100 */ pub const _REG_FIFO_CNT_RO_RECV_EMPTY_MASK: u16 = 1 << _REG_FIFO_CNT_RO_RECV_EMPTY_SHIFT;
/** 0x0008 */ pub const  REG_FIFO_CNT_CLEAR_SEND_QUEUE_MASK: u16 = 1 << REG_FIFO_CNT_CLEAR_SEND_QUEUE_SHIFT;
/** 0x0004 */ pub const _REG_FIFO_CNT_IRQ_UPON_SEND_EMPTY_MASK: u16 = 1 << _REG_FIFO_CNT_IRQ_UPON_SEND_EMPTY_SHIFT;
/** 0x0002 */ pub const _REG_FIFO_CNT_RO_SEND_FULL_MASK: u16 = 1 << _REG_FIFO_CNT_RO_SEND_FULL_SHIFT;
/** 0x0001 */ pub const _REG_FIFO_CNT_RO_SEND_EMPTY_MASK: u16 = 1 << _REG_FIFO_CNT_RO_SEND_EMPTY_SHIFT;

/// ARM7-only
pub const _REG_IE_CHECK_WIRELESS_CHIP_SHIFT: usize = 24;
/// ARM7-only
pub const _REG_IE_SPI_TRANSFERRED_ONE_BYTE_SHIFT: usize = 23;
/// ARM7-only
pub const _REG_IE_LID_OPENED_SHIFT: usize = 22;
/// ARM9-only
pub const  REG_IE_GEOMETRY_FIFO_LEVEL_SHIFT: usize = 21;
pub const _REG_IE_NDS_SLOT_IREQ_PIN_SHIFT: usize = 20;
pub const _REG_IE_NDS_CARD_TRANSFER_DONE_SHIFT: usize = 19;
pub const  REG_IE_FIFO_RECV_NON_EMPTY_SHIFT: usize = 18;
pub const _REG_IE_FIFO_SEND_EMPTY_SHIFT: usize = 17;
pub const _REG_IE_SYNC_IREQ_SHIFT: usize = 16;
pub const _REG_IE_GBA_SLOT_IREQ_PIN_SHIFT: usize = 13;
pub const _REG_IE_KEYINPUT_MATCH_SHIFT: usize = 12;
pub const  REG_IE_DMA_3_FINISHED_SHIFT: usize = 11;
pub const _REG_IE_DMA_2_FINISHED_SHIFT: usize = 10;
pub const _REG_IE_DMA_1_FINISHED_SHIFT: usize = 9;
pub const  REG_IE_DMA_0_FINISHED_SHIFT: usize = 8;
/// ARM7-only
pub const _REG_IE_RTC_SHIFT: usize = 7; // the serial (link cable) interrupt on GBA
pub const  REG_IE_TIMER_3_OVFLOW_SHIFT: usize = 6;
pub const _REG_IE_TIMER_2_OVFLOW_SHIFT: usize = 5;
pub const _REG_IE_TIMER_1_OVFLOW_SHIFT: usize = 4;
pub const  REG_IE_TIMER_0_OVFLOW_SHIFT: usize = 3;
pub const _REG_IE_VCOUNT_MATCH_SHIFT: usize = 2;
pub const _REG_IE_HBLANK_SHIFT: usize = 1;
pub const _REG_IE_VBLANK_SHIFT: usize = 0;

/** 0x01000000 
 *  ARM7-only  */ pub const _REG_IE_CHECK_WIRELESS_CHIP_MASK: u32 = 1 << _REG_IE_CHECK_WIRELESS_CHIP_SHIFT;
/** 0x00800000 
 *  ARM7-only  */ pub const _REG_IE_SPI_TRANSFERRED_ONE_BYTE_MASK: u32 = 1 << _REG_IE_SPI_TRANSFERRED_ONE_BYTE_SHIFT;
/** 0x00400000 
 *  ARM7-only  */ pub const _REG_IE_LID_OPENED_MASK: u32 = 1 << _REG_IE_LID_OPENED_SHIFT;
/** 0x00200000 
 *  ARM9-only  */ pub const _REG_IE_GEOMETRY_FIFO_LEVEL_MASK: u32 = 1 << REG_IE_GEOMETRY_FIFO_LEVEL_SHIFT;
/** 0x00100000 */ pub const _REG_IE_NDS_SLOT_IREQ_PIN_MASK: u32 = 1 << _REG_IE_NDS_SLOT_IREQ_PIN_SHIFT;
/** 0x00080000 */ pub const _REG_IE_NDS_CARD_TRANSFER_DONE_MASK: u32 = 1 << _REG_IE_NDS_CARD_TRANSFER_DONE_SHIFT;
/** 0x00040000 */ pub const  REG_IE_FIFO_RECV_NON_EMPTY_MASK: u32 = 1 << REG_IE_FIFO_RECV_NON_EMPTY_SHIFT;
/** 0x00020000 */ pub const _REG_IE_FIFO_SEND_EMPTY_MASK: u32 = 1 << _REG_IE_FIFO_SEND_EMPTY_SHIFT;
/** 0x00010000 */ pub const _REG_IE_SYNC_IREQ_MASK: u32 = 1 << _REG_IE_SYNC_IREQ_SHIFT;
/** 0x00002000 */ pub const _REG_IE_GBA_SLOT_IREQ_PIN_MASK: u32 = 1 << _REG_IE_GBA_SLOT_IREQ_PIN_SHIFT;
/** 0x00001000 */ pub const _REG_IE_KEYINPUT_MATCH_MASK: u32 = 1 << _REG_IE_KEYINPUT_MATCH_SHIFT;
/** 0x00000800 */ pub const _REG_IE_DMA_3_FINISHED_MASK: u32 = 1 << REG_IE_DMA_3_FINISHED_SHIFT;
/** 0x00000400 */ pub const _REG_IE_DMA_2_FINISHED_MASK: u32 = 1 << _REG_IE_DMA_2_FINISHED_SHIFT;
/** 0x00000200 */ pub const _REG_IE_DMA_1_FINISHED_MASK: u32 = 1 << _REG_IE_DMA_1_FINISHED_SHIFT;
/** 0x00000100 */ pub const _REG_IE_DMA_0_FINISHED_MASK: u32 = 1 << REG_IE_DMA_0_FINISHED_SHIFT;
/** 0x00000080 
 *  ARM7-only  */ pub const _REG_IE_RTC_MASK: u32 = 1 << _REG_IE_RTC_SHIFT; // the serial (link cable) interrupt on GBA
/** 0x00000040 */ pub const _REG_IE_TIMER_3_OVFLOW_MASK: u32 = 1 << REG_IE_TIMER_3_OVFLOW_SHIFT;
/** 0x00000020 */ pub const _REG_IE_TIMER_2_OVFLOW_MASK: u32 = 1 << _REG_IE_TIMER_2_OVFLOW_SHIFT;
/** 0x00000010 */ pub const _REG_IE_TIMER_1_OVFLOW_MASK: u32 = 1 << _REG_IE_TIMER_1_OVFLOW_SHIFT;
/** 0x00000008 */ pub const _REG_IE_TIMER_0_OVFLOW_MASK: u32 = 1 << REG_IE_TIMER_0_OVFLOW_SHIFT;
/** 0x00000004 */ pub const _REG_IE_VCOUNT_MATCH_MASK: u32 = 1 << _REG_IE_VCOUNT_MATCH_SHIFT;
/** 0x00000002 */ pub const _REG_IE_HBLANK_MASK: u32 = 1 << _REG_IE_HBLANK_SHIFT;
/** 0x00000001 */ pub const _REG_IE_VBLANK_MASK: u32 = 1 << _REG_IE_VBLANK_SHIFT;

pub const _REG_IE_ARM7_IRQ_SOURCES_MAX: usize = _REG_IE_CHECK_WIRELESS_CHIP_SHIFT + 1;
pub const  REG_IE_ARM9_IRQ_SOURCES_MAX: usize = REG_IE_GEOMETRY_FIFO_LEVEL_SHIFT + 1;

pub const _REG_SYNC_INBOUND_IREQ_ALLOW_SHIFT: usize = 14;
pub const _REG_SYNC_IREQ_SHIFT: usize = 13;
pub const  REG_SYNC_THIS_SIDE_SHIFT: usize = 8;
pub const  REG_SYNC_RO_THAT_SIDE_SHIFT: usize = 0;

/** 0x4000 */ pub const _REG_SYNC_INBOUND_IREQ_ALLOW_MASK: u16 = 1 << _REG_SYNC_INBOUND_IREQ_ALLOW_SHIFT;
/** 0x2000 */ pub const _REG_SYNC_IREQ_MASK: u16 = 1 << _REG_SYNC_IREQ_SHIFT;
/** 0x0F00 */ pub const  REG_SYNC_THIS_SIDE_MASK: u16 = 0xF << REG_SYNC_THIS_SIDE_SHIFT;
/** 0x000F */ pub const  REG_SYNC_RO_THAT_SIDE_MASK: u16 = 0xF << REG_SYNC_RO_THAT_SIDE_SHIFT;

pub use crate::sys::SYS_AREA_START;
pub use crate::sys::SYS_AREA_SYSTEM_WORK;
pub const TIMERS_COUNT: usize = REG_IE_TIMER_3_OVFLOW_SHIFT - REG_IE_TIMER_0_OVFLOW_SHIFT + 1;

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
/** 0x00000003 */ pub const OS_CONSOLE_SIZE_MASK: u32 = 0x00000003;
pub const OS_CONSOLE_SIZE_4MB: u32 = 0x00000001;

/* MMAP.H */

/// Beyond this is private memory reserved for the hardware debugger when the game is
/// built and run on the Debug Console with 8 MiB Main RAM.
pub const HW_MAIN_MEM_DEBUGGER_OFFSET: u32 = 0x00700000;

/* REGNAME.H */

pub const  PXI_PROC_ARM9: usize = 0;
pub const _PXI_PROC_ARM7: usize = 1;

