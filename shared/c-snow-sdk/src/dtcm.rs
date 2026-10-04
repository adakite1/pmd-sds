cfg_select! {
armv5te => {

/* ====== ARM9 ====== */

use crate::{Global, consts::REG_IE_ARM9_IRQ_SOURCES_MAX, os::{OSThreadQueue, OS_IrqDummy, OSi_IrqDma0, OSi_IrqDma1, OSi_IrqDma2, OSi_IrqDma3, OSi_IrqTimer0, OSi_IrqTimer1, OSi_IrqTimer2, OSi_IrqTimer3}};

#[unsafe(link_section = ".data.dtcm.origaddr.027E0000")]
pub static OS_IRQTable: Global<[Option<extern "C" fn()>; REG_IE_ARM9_IRQ_SOURCES_MAX]> = Global::new([
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OSi_IrqTimer0),
    Some(OSi_IrqTimer1),
    Some(OSi_IrqTimer2),
    Some(OSi_IrqTimer3),
    Some(OS_IrqDummy),
    Some(OSi_IrqDma0),
    Some(OSi_IrqDma1),
    Some(OSi_IrqDma2),
    Some(OSi_IrqDma3),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
    Some(OS_IrqDummy),
]);

#[unsafe(link_section = ".bss.dtcm.origaddr.027E0060")]
pub static OSi_IrqThreadQueue: Global<OSThreadQueue> = Global::new(OSThreadQueue {
    head: core::ptr::null_mut(),
    tail: core::ptr::null_mut(),
});

/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */


/*cfg_select*/ } _
=> {} }

