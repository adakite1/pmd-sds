use crate::Global;

cfg_select! {
armv5te => {

/* ====== ARM9 ====== */

#[unsafe(link_section = ".text.origaddr.0207A36C")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_Init() {
    OS_InitArena();
}

#[unsafe(link_section = ".text.origaddr.0207A3B4")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_InitArena() {

}

#[unsafe(link_section = ".bss.origaddr.022B98C4")]
pub static OSi_Initialized: Global<u32> = Global::new(0);

#[unsafe(link_section = ".bss.origaddr.022B98C8")]
pub static OSi_MAINEX_ENABLE: Global<u32> = Global::new(0); /* enable extended Main RAM 8 MiB (debug console only) */

/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */





/*cfg_select*/ } _
=> {} }

