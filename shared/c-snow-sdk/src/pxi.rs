use raw_traversal::raw_field;

use crate::{Global, consts::{PXI_MAX_FIFO_MESSAGE_TAG, PXI_PROC, REG_FIFO_CNT_CLEAR_SEND_QUEUE_MASK, REG_FIFO_CNT_ENABLE_MASK, REG_FIFO_CNT_ERROR_MASK, REG_FIFO_CNT_IRQ_UPON_RECV_NON_EMPTY_MASK, REG_IE_FIFO_RECV_NON_EMPTY_MASK, REG_SYNC_RO_THAT_SIDE_MASK, REG_SYNC_THIS_SIDE_SHIFT}, os::{OS_AckIrqRequests, OS_CpuDisableIrq, OS_CpuRestoreIrq, OS_EnableIrqSources, OS_SetIrqFunction}, read, reg::{REG_FIFO_CNT, REG_SYNC}, sys::{OSSystemWork, SYS_AREA_SYSTEM_WORK}, write};

/*(asm-corpus-filename: arm7/wram/sub_037FE2B4, objdump-symbol-name: arm7_bin/PXI_Init)DONE('pxi_init')*/
/*(asm-corpus-filename: PXI_Init, objdump-symbol-name: arm9_bin/PXI_Init)DONE('pxi_init', us, eu, jp)*/
#[inline(never)]
#[cfg_attr(armv4t, unsafe(link_section = ".text.wram.origaddr.037FE2B4"))]
#[cfg_attr(armv5te, unsafe(link_section = ".text.origaddr.0207D9A4"))]
#[unsafe(no_mangle)]
pub extern "C" fn PXI_Init() {
    PXI_InitFifo();
}

/*(asm-corpus-filename: arm7/wram/sub_037FE2C0{ignore, lines(8, 11..=28)}, objdump-symbol-name: inline(=arm7_bin/init_fifo_setup))DONE('pxi_fifo')*/
/*(asm-corpus-filename: arm9/main/PXI_InitFifo{ignore, lines(10..=28)}, objdump-symbol-name: inline(=arm9_bin/init_fifo_setup))DONE('pxi_fifo', us, eu, jp)*/
#[inline(always)]
fn init_fifo_setup() {
    raw_field!(SYS_AREA_SYSTEM_WORK as *mut OSSystemWork, final mut pxiHandleChecker[PXI_PROC]).set(0);
    for i in 0..PXI_MAX_FIFO_MESSAGE_TAG {
        raw_field!(PXIi_FifoRecvCallbackTable.as_ptr(), final [i]).set(None);
    }
    write(REG_FIFO_CNT, REG_FIFO_CNT_ENABLE_MASK | REG_FIFO_CNT_ERROR_MASK | REG_FIFO_CNT_IRQ_UPON_RECV_NON_EMPTY_MASK | REG_FIFO_CNT_CLEAR_SEND_QUEUE_MASK);
    OS_AckIrqRequests(REG_IE_FIFO_RECV_NON_EMPTY_MASK);
    OS_SetIrqFunction(REG_IE_FIFO_RECV_NON_EMPTY_MASK, Some(PXIi_IrqFifoRecvNonEmpty));
    OS_EnableIrqSources(REG_IE_FIFO_RECV_NON_EMPTY_MASK);
}

cfg_select! {
armv5te => {

/* ====== ARM9 ====== */

use crate::consts::{ARM9_FIFO_HS_ACCEPT_SUCCESS_AFTER, ARM9_FIFO_HS_SPINWAIT_FOR_SUBPROC_NEXT_NUMBER_BEFORE_RESET};

/*(asm-corpus-filename: arm9/main/PXI_InitFifo{ignore, lines(29..=54)}, objdump-symbol-name: inline(=arm9_bin/init_fifo_handshake_mainproc, all~~4))DONE('pxi_fifo', us, eu, jp)*/
#[inline(always)]
fn init_fifo_handshake_mainproc() {
    let mut i = 0;
    loop {
        let remote = read(REG_SYNC) & REG_SYNC_RO_THAT_SIDE_MASK;
        write(REG_SYNC, remote << REG_SYNC_THIS_SIDE_SHIFT);
        if remote == 0 && i > ARM9_FIFO_HS_ACCEPT_SUCCESS_AFTER {
            break;
        }
        let mut timeout = ARM9_FIFO_HS_SPINWAIT_FOR_SUBPROC_NEXT_NUMBER_BEFORE_RESET;
        while read(REG_SYNC) & REG_SYNC_RO_THAT_SIDE_MASK == remote {
            if timeout == 0 {
                i = 0;
                break;
            }
            timeout -= 1;
        }
        i += 1;
    }
}

/*(asm-corpus-filename: PXI_InitFifo, objdump-symbol-name: arm9_bin/PXI_InitFifo)DONE('pxi_fifo', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207D9B0")]
#[unsafe(no_mangle)]
pub extern "C" fn PXI_InitFifo() {
    let prev = OS_CpuDisableIrq();
    if PXIi_InitializedFifo.get() == 0 {
        PXIi_InitializedFifo.set(1);
        init_fifo_setup();
        init_fifo_handshake_mainproc();
    }
    OS_CpuRestoreIrq(prev);
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207DBA8")]
#[unsafe(no_mangle)]
pub extern "C" fn PXIi_IrqFifoRecvNonEmpty() {
    
}

#[unsafe(link_section = ".bss.origaddr.022BB564")]
pub static PXIi_InitializedFifo: Global<u16> = Global::new(0);

#[unsafe(link_section = ".bss.origaddr.022BB568")]
pub static PXIi_FifoRecvCallbackTable: Global<[Option<extern "C" fn(u32, u32, u32)>; PXI_MAX_FIFO_MESSAGE_TAG]> = Global::new([None; PXI_MAX_FIFO_MESSAGE_TAG]);

/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */

use crate::{consts::{ARM7_FIFO_HS_MAINPROC_MUST_RESPOND_WITHIN, ARM7_FIFO_HS_SUCCESS_AFTER}, os::OS_SpinWait};

/*(asm-corpus-filename: arm7/wram/sub_037FE2C0{ignore, lines(29..=44)}, objdump-symbol-name: inline(=arm7_bin/init_fifo_handshake_subproc, all~~4))DONE('pxi_fifo')*/
#[inline(always)]
fn init_fifo_handshake_subproc() {
    let mut i = ARM7_FIFO_HS_SUCCESS_AFTER as i32;
    while i >= 0 {
        write(REG_SYNC, (i as u16) << REG_SYNC_THIS_SIDE_SHIFT);
        OS_SpinWait(ARM7_FIFO_HS_MAINPROC_MUST_RESPOND_WITHIN);
        if (read(REG_SYNC) & REG_SYNC_RO_THAT_SIDE_MASK) as i32 != i {
            i = ARM7_FIFO_HS_SUCCESS_AFTER as i32;
        }
        i -= 1;
    }
}

/*(asm-corpus-filename: arm7/wram/sub_037FE2C0, objdump-symbol-name: arm7_bin/PXI_InitFifo)DONE('pxi_fifo')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.037FE2C0")]
#[unsafe(no_mangle)]
pub extern "C" fn PXI_InitFifo() {
    let prev = OS_CpuDisableIrq();
    if PXIi_InitializedFifo.get() == 0 {
        PXIi_InitializedFifo.set(1);
        init_fifo_setup();
        init_fifo_handshake_subproc();
    }
    OS_CpuRestoreIrq(prev);
}

#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.037FE4A8")]
#[unsafe(no_mangle)]
pub extern "C" fn PXIi_IrqFifoRecvNonEmpty() {
    
}

#[unsafe(link_section = ".bss.wram.origaddr.0380785C")]
pub static PXIi_InitializedFifo: Global<u16> = Global::new(0);

#[unsafe(link_section = ".bss.wram.origaddr.03807860")]
pub static PXIi_FifoRecvCallbackTable: Global<[Option<extern "C" fn(u32, u32, u32)>; PXI_MAX_FIFO_MESSAGE_TAG]> = Global::new([None; PXI_MAX_FIFO_MESSAGE_TAG]);

/*cfg_select*/ } _
=> {} }

