cfg_select! {
armv5te => {

/* ====== ARM9 ====== */

use raw_traversal::raw_field;

use crate::{Global, consts::{ARM9_FIFO_HS_ACCEPT_SUCCESS_AFTER, ARM9_FIFO_HS_SPINWAIT_FOR_SUBPROC_NEXT_NUMBER_BEFORE_RESET, REG_IE_FIFO_RECV_NON_EMPTY_MASK, PXI_MAX_FIFO_MESSAGE_TAG, PXI_PROC_ARM9, REG_FIFO_CNT_ENABLE_MASK, REG_FIFO_CNT_ERROR_MASK, REG_FIFO_CNT_IRQ_UPON_RECV_NON_EMPTY_MASK, REG_FIFO_CNT_CLEAR_SEND_QUEUE_MASK, REG_SYNC_RO_THAT_SIDE_MASK, REG_SYNC_THIS_SIDE_SHIFT}, os::{OS_AckIrqRequests, OS_CpuDisableIrq, OS_CpuRestoreIrq, OS_EnableIrqSources, OS_SetIrqFunction}, read, reg::{REG_FIFO_CNT, REG_SYNC}, sys::{OSSystemWork, SYS_AREA_SYSTEM_WORK}, write};

/*(asm-corpus-filename: PXI_Init, objdump-symbol-name: arm9_bin/PXI_Init)DONE('pxi_init', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207D9A4")]
#[unsafe(no_mangle)]
pub extern "C" fn PXI_Init() {
    PXI_InitFifo();
}

/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */


/*cfg_select*/ } _
=> {} }

