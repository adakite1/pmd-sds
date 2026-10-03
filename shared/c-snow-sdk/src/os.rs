use raw_traversal::raw_field;

use crate::{Global, consts::{HW_MAIN_MEM_DEBUGGER_OFFSET, OSi_IRQ_CALLBACK_MAX, REG_OS_IE_D0_SHIFT, REG_OS_IE_D3_SHIFT, REG_OS_IE_T0_SHIFT, REG_OS_IE_T3_SHIFT, OS_ARENA_DTCM, OS_ARENA_ITCM, OS_ARENA_MAIN, OS_ARENA_MAINEX, OS_ARENA_SYS_AREA_SHARED, OS_ARENA_WRAM_SHARED_ARM9, OS_CONSOLE_DEV_CARD, OS_CONSOLE_NITRO, OS_CONSOLE_SIZE_4MB, OS_CONSOLE_SIZE_MASK, OSi_CONSOLE_NOT_DETECT}, cp15_size_encoding, linker_addr, read, reg::{REG_IE, REG_IF, REG_IME}, set_cp15_region, write, sys::{HW_CARD_ROM_HEADER_SIZE, HW_DOWNLOAD_PARAMETER_SIZE, OSArenaInfo, SYS_AREA_ARENA_INFO_BUF, SYS_AREA_START, red::hw_red_reserved}};

#[repr(C)]
pub struct OSThreadQueue {
    pub head: *mut core::ffi::c_void,
    pub tail: *mut core::ffi::c_void,

    //TODO:
}

#[repr(C)]
pub struct OSIrqCallbackInfo {
    pub func: Option<extern "C" fn(*mut core::ffi::c_void)>,
    pub enable: u32,
    pub arg: *mut core::ffi::c_void,
}

#[repr(C)]
pub struct OSIrqCallbackInfoTable {
    pub dma_callbacks: [OSIrqCallbackInfo; REG_OS_IE_D3_SHIFT - REG_OS_IE_D0_SHIFT + 1],
    pub timer_callbacks: [OSIrqCallbackInfo; REG_OS_IE_T3_SHIFT - REG_OS_IE_T0_SHIFT + 1],
}
const _: () = assert!(core::mem::size_of::<OSIrqCallbackInfoTable>() / core::mem::size_of::<OSIrqCallbackInfo>() == OSi_IRQ_CALLBACK_MAX);

cfg_select! {
armv5te => {

/* ====== ARM9 ====== */

use crate::dtcm::OSi_IrqThreadQueue;

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078974")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_IrqDummy() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A00")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqDma0() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A10")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqDma1() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A20")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqDma2() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A30")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqDma3() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A40")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqTimer0() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A50")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqTimer1() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A60")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqTimer2() {
    
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A70")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_IrqTimer3() {
    
}

/*(asm-corpus-filename: OS_InitIrqTable, objdump-symbol-name: arm9_bin/OS_InitIrqTable)DONE('os_interrupt')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078A80")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_InitIrqTable() {
    raw_field!(OSi_IrqThreadQueue.as_ptr(), final mut tail).set(core::ptr::null_mut());
    raw_field!(OSi_IrqThreadQueue.as_ptr(), final mut head).set(core::ptr::null_mut());
}

#[inline(always)]
pub fn OS_DisableIrq() -> u16 {
    let prev = read(REG_IME);
    write(REG_IME, 0);
    prev
}

#[inline(always)]
pub fn OS_RestoreIrq(enable: u16) -> u16 {
    let prev = read(REG_IME);
    write(REG_IME, enable);
    prev
}

/*(asm-corpus-filename: OSi_EnterDmaCallback, objdump-symbol-name: arm9_bin/OSi_EnterDmaCallback)DONE('os_interrupt', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078BAC")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_EnterDmaCallback(dma_no: u32, callback: Option<extern "C" fn(*mut core::ffi::c_void)>, arg: *mut core::ffi::c_void) {
    let imask: u32 = 1 << (REG_OS_IE_D0_SHIFT as u32 + dma_no);
    let info = raw_field!(OSi_IrqCallbackInfo.as_ptr(), mut dma_callbacks[dma_no as usize]);
    raw_field!(info, final mut func).set(callback);
    raw_field!(info, final mut arg).set(arg);
    raw_field!(info, final mut enable).set(OS_EnableIrqMask(imask) & imask);
}

/*(asm-corpus-filename: OSi_EnterTimerCallback, objdump-symbol-name: arm9_bin/OSi_EnterTimerCallback)DONE('os_interrupt', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078BF4")]
#[unsafe(no_mangle)]
pub extern "C" fn OSi_EnterTimerCallback(timer_no: u32, callback: Option<extern "C" fn(*mut core::ffi::c_void)>, arg: *mut core::ffi::c_void) {
    let imask: u32 = 1 << (REG_OS_IE_T0_SHIFT as u32 + timer_no);
    let info = raw_field!(OSi_IrqCallbackInfo.as_ptr(), mut timer_callbacks[timer_no as usize]);
    raw_field!(info, final mut func).set(callback);
    raw_field!(info, final mut arg).set(arg);
    OS_EnableIrqMask(imask);
    raw_field!(info, final mut enable).set(1);
}

/*(asm-corpus-filename: OS_SetIrqMask, objdump-symbol-name: arm9_bin/OS_SetIrqMask)DONE('os_interrupt')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078C3C")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_SetIrqMask(mask: u32) -> u32 {
    let ime = OS_DisableIrq();
    let prev = read(REG_IE);
    write(REG_IE, mask);
    OS_RestoreIrq(ime); //NOTE: An extra ldrh r0, [r2] (like in the original asm) here because the read in OS_RestoreIrq is volatile even if the read value is not used
    prev
}

/*(asm-corpus-filename: OS_EnableIrqMask, objdump-symbol-name: arm9_bin/OS_EnableIrqMask)DONE('os_interrupt')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078C68")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_EnableIrqMask(mask: u32) -> u32 {
    let ime = OS_DisableIrq();
    let prev = read(REG_IE);
    write(REG_IE, prev | mask);
    OS_RestoreIrq(ime);
    prev
}

/*(asm-corpus-filename: OS_DisableIrqMask, objdump-symbol-name: arm9_bin/OS_DisableIrqMask)DONE('os_interrupt')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078C98")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_DisableIrqMask(mask: u32) -> u32 {
    let ime = OS_DisableIrq();
    let prev = read(REG_IE);
    write(REG_IE, prev & !mask);
    OS_RestoreIrq(ime);
    prev
}

/*(asm-corpus-filename: OS_ResetRequestIrqMask, objdump-symbol-name: arm9_bin/OS_ResetRequestIrqMask)DONE('os_interrupt')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078CC8")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_ResetRequestIrqMask(mask: u32) -> u32 {
    let ime = OS_DisableIrq();
    let prev = read(REG_IF);
    write(REG_IF, mask);
    OS_RestoreIrq(ime);
    prev
}

/*(asm-corpus-filename: OS_SetIrqStackChecker, objdump-symbol-name: arm9_bin/OS_SetIrqStackChecker)DONE('os_interrupt')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02078CF4")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_SetIrqStackChecker() {
    write(linker_addr(&raw const __irq_stack_top) as *mut u32, 0xFDDB597D);
    write(linker_addr(&raw const __irq_stack_bottom) as *mut u32, 0x7BF9DD5B);
}

/*(asm-corpus-filename: OS_GetConsoleType, objdump-symbol-name: arm9_bin/OS_GetConsoleType)DONE('os_emulator', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.02079DA0")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_GetConsoleType() -> u32 {
    OSi_ConsoleTypeCache.set(const { OS_CONSOLE_NITRO | OS_CONSOLE_DEV_CARD | OS_CONSOLE_SIZE_4MB })
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A36C")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_Init() {
    OS_InitArena();

    //TODO:
}

/*(asm-corpus-filename: OS_InitArena, objdump-symbol-name: arm9_bin/OS_InitArena)DONE('os_arena', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A3B4")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_InitArena() {
    if OSi_Initialized.get() != 0 {
        return;
    }
    OSi_Initialized.set(1);

    OS_SetArenaHi(OS_ARENA_MAIN, OS_GetInitArenaHi(OS_ARENA_MAIN));
    OS_SetArenaLo(OS_ARENA_MAIN, OS_GetInitArenaLo(OS_ARENA_MAIN));

    OS_SetArenaLo(OS_ARENA_MAINEX, 0);
    OS_SetArenaHi(OS_ARENA_MAINEX, 0);

    OS_SetArenaHi(OS_ARENA_ITCM, OS_GetInitArenaHi(OS_ARENA_ITCM));
    OS_SetArenaLo(OS_ARENA_ITCM, OS_GetInitArenaLo(OS_ARENA_ITCM));

    OS_SetArenaHi(OS_ARENA_DTCM, OS_GetInitArenaHi(OS_ARENA_DTCM));
    OS_SetArenaLo(OS_ARENA_DTCM, OS_GetInitArenaLo(OS_ARENA_DTCM));

    OS_SetArenaHi(OS_ARENA_SYS_AREA_SHARED, OS_GetInitArenaHi(OS_ARENA_SYS_AREA_SHARED));
    OS_SetArenaLo(OS_ARENA_SYS_AREA_SHARED, OS_GetInitArenaLo(OS_ARENA_SYS_AREA_SHARED));

    OS_SetArenaHi(OS_ARENA_WRAM_SHARED_ARM9, OS_GetInitArenaHi(OS_ARENA_WRAM_SHARED_ARM9));
    OS_SetArenaLo(OS_ARENA_WRAM_SHARED_ARM9, OS_GetInitArenaLo(OS_ARENA_WRAM_SHARED_ARM9));
}

/*(asm-corpus-filename: OS_InitArenaEx, objdump-symbol-name: arm9_bin/OS_InitArenaEx)DONE('os_arena', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A4B8")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_InitArenaEx() {
    OS_SetArenaHi(OS_ARENA_MAINEX, OS_GetInitArenaHi(OS_ARENA_MAINEX));
    OS_SetArenaLo(OS_ARENA_MAINEX, OS_GetInitArenaLo(OS_ARENA_MAINEX));

    if OSi_MAINEX_ENABLE.get() == 0 || OS_GetConsoleType() & OS_CONSOLE_SIZE_MASK == OS_CONSOLE_SIZE_4MB {
        OS_SetProtectionRegionsRetail();
    }
}

/// This block might feel strange, why are we changing the MPU settings again?
///
/// It makes sense however once you work it out. First, recall that arm7 primarily
/// accesses memory, always, through the upper alias. However, we mentioned before in
/// `_start` that since on a retail nds the Main RAM is mirrored, region 2 of the MPU
/// protecting arm9 from touching arm7's private Main RAM, does not prevent it from
/// accidentally aliasing the same memory *anyways* from the *lower* alias.
///
/// I said before that thus, the lower alias is only avoided by convention on arm9. **This tightens that.**
///
/// From the ARM946E-S Technical Reference Manual:
///
/// > If the address issued by the processor falls outside any of the defined regions, the
/// > ARM946E-S protection unit is hard-wired to abort the access. You can override this
/// > behavior by programming region 0 to be a 4GB background region. In this way, if the
/// > address does not fall into any of the other seven regions, the access is controlled by the
/// > attributes you have specified for region 0.
///
/// In this game, region 0 is *NOT* the blanket background region described there; instead, there
/// aren't any real blanket background regions at all. The first region covers 04000000-07FFFFFF, so
/// all the I/O and VRAM and stuff like that, not the Main RAM. The Main RAM "blanket" would be
/// the second region (confusingly region 1), which goes from 02000000-027FFFFF.
///
/// What that means when combined with the TRM detail above is that on a retail console with only
/// 4 MiB of Main RAM, **if we shrink Region 1 to 4 MiB, we don't need Region 2 to deliberately set
/// `NoAccess`/`NoAccess` on the upper mirror at all.** Any memory range in the upper alias we have not
/// carved out explicitly with a higher-numbered region, becomes `NoAccess` by default.
///
/// Then, with Region 2 now free for other uses, we can use it to close the leak from before.
/// We move it to 0x023E0000 and protect the lower alias from ARM9 access as well, thus giving
/// us a truly ARM7-private Main RAM block.
///
/// Note that this is only true on the retail console. In an 8 MiB Main RAM debug console,
/// this is all skipped.
/*(asm-corpus-filename: [OS_SetProtectionRegion1, OS_SetProtectionRegion2], objdump-symbol-name: inline(=arm9_bin/OS_SetProtectionRegionsRetail, OS_InitArenaEx))DONE('os_protectionRegion')*/
#[inline(always)]
pub extern "C" fn OS_SetProtectionRegionsRetail() {
    unsafe {
        set_cp15_region!(
            region = c1,
            base = 0x02000000,
            /* 0x02000000, 4 MiB */
            size_encoding = cp15_size_encoding(4 * 1024 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c2,
            base = 0x023E0000,
            /* 0x023E0000, 128 KiB */
            size_encoding = cp15_size_encoding(128 * 1024),
            enabled = true,
        );
    }
}

/*(asm-corpus-filename: OS_GetArenaHi, objdump-symbol-name: arm9_bin/OS_GetArenaHi)DONE('os_arena')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A524")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_GetArenaHi(arena_index: u32) -> u32 {
    raw_field!(SYS_AREA_ARENA_INFO_BUF as *const OSArenaInfo, final hi[arena_index as usize]).get()
}

/*(asm-corpus-filename: OS_GetArenaLo, objdump-symbol-name: arm9_bin/OS_GetArenaLo)DONE('os_arena')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A538")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_GetArenaLo(arena_index: u32) -> u32 {
    raw_field!(SYS_AREA_ARENA_INFO_BUF as *const OSArenaInfo, final lo[arena_index as usize]).get()
}

unsafe extern "C" {
    static __irq_stack_top: u8;
    static __irq_stack_bottom: u8;
    static __main_ram_top_arm9: u8;
    static __main_ram_base: u8;
    static __itcm_heap_end: u8;
    static __additional_heap_space_in_dtcm_hi: u8;
    static __arm7_wram_base: u8;
    static __static_data_end: u8;
    static __itcm_heap_start: u8;
    static __additional_heap_space_in_dtcm_lo: u8;
}

/*(asm-corpus-filename: OS_GetInitArenaHi, objdump-symbol-name: arm9_bin/OS_GetInitArenaHi)DONE('os_arena', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A54C")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_GetInitArenaHi(arena_index: u32) -> u32 {
    match arena_index {
        7.. => {
            /* no such arena */
            0
        },
        0 => {
            linker_addr(&raw const __main_ram_top_arm9)
        },
        1 => {
            /* no such arena */
            0
        },
        2 => {
            if OSi_MAINEX_ENABLE.get() == 0 {
                return 0;
            }
            if OS_GetConsoleType() & OS_CONSOLE_SIZE_MASK != OS_CONSOLE_SIZE_4MB {
                /* big debug console with 8 MiB Main RAM, everything starting from
                    the lower half, in the middle of it starting from where usually the lower
                    mirror of the arm7 Main RAM would be at 0x023E0000 (because separate
                    memory on debug console, large RAM remember), all the way to 0x02700000
                    (which stops short of where arm9's Main RAM upper-mirror would end
                    at 0x027E0000 since there's also some space reserved for the
                    hardware debugger unit), is free space for this arena. */
                return linker_addr(&raw const __main_ram_base) + HW_MAIN_MEM_DEBUGGER_OFFSET;
            }
            0
        },
        3 => {
            linker_addr(&raw const __itcm_heap_end)
        },
        4 => {
            linker_addr(&raw const __additional_heap_space_in_dtcm_hi)
        },
        5 => {
            /* This does not make any sense. Why would we base it off of *minus* of all operations? You might wonder.
                In addresses, additions and subtractions are supposed to tell you relationships. But here,
                semantically, "it has to be two metadata blocks *before* RED" makes zero sense.
                What relationship does "RED" have with the two metadata blocks? Does it use them?
                Does it seek for it with negative offset? Does it like having it for company?
                Because as far as I can tell, the space, the 0x160 + 0x20 space it creates before "RED"
                here, *ISN'T EVEN FILLED WITH WHAT IT SAYS ON THE TIN.*
                It's *dead*. The region there, 0x27FF680..0x027FF800? There are no references to it in the BIOS and firmware,
                in the game, anywhere, for the lower mirror too. So it's not just that the implied dependency of "RED needs to
                come right after the two blocks" is not true, not only does *RED* not access this, not even *ANYTHING ELSE* accesses
                this region. If you're wondering about the 0x160 + 0x20 regions, that's *somewhere else*, at HW_CARD_ROM_HEADER and
                HW_DOWNLOAD_PARAMETER respectively (0x027FFA80 with size 0x160, and then 0x20 immediately after for the latter.)
                
                If I had to guess, at some point the layout was different, and the carved out gap space here *was* where those two
                were supposed to go. Then it got changed, and no one updated this. */
            hw_red_reserved(linker_addr(&raw const __main_ram_base)) - HW_CARD_ROM_HEADER_SIZE - HW_DOWNLOAD_PARAMETER_SIZE
        },
        6 => {
            /* How much heap in wram for arm9, see __arm7_wram_base comment in
                arm9.ld for why this is effectively zero size in this game. */
            linker_addr(&raw const __arm7_wram_base)
        },
    }
}

/*(asm-corpus-filename: OS_GetInitArenaLo, objdump-symbol-name: arm9_bin/OS_GetInitArenaLo)DONE('os_arena', us, eu, jp)*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A630")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_GetInitArenaLo(arena_index: u32) -> u32 {
    match arena_index {
        7.. => {
            /* no such arena */
            0
        },
        0 => {
            /* Starting from the end of the arm9 static code (where the previous start of the autoloaded data was)
                can be used as a heap arena by this point because the autoloaded content has long since already been
                copied over to their respective VMA positions. */
            linker_addr(&raw const __static_data_end)
        },
        1 => {
            /* no such arena */
            0
        },
        2 => {
            if OSi_MAINEX_ENABLE.get() == 0 {
                return 0;
            }
            if OS_GetConsoleType() & OS_CONSOLE_SIZE_MASK != OS_CONSOLE_SIZE_4MB {
                return linker_addr(&raw const __main_ram_top_arm9);
            }
            0
        },
        3 => {
            linker_addr(&raw const __itcm_heap_start)
        },
        4 => {
            linker_addr(&raw const __additional_heap_space_in_dtcm_lo)
        },
        5 => {
            /* The start of the System and SDK Area, all the way up to 0x027FF67F in the original game,
                is the shared heap arena, accessible from both CPUs. */
            SYS_AREA_START
        },
        6 => {
            /* How much heap in wram for arm9, see __arm7_wram_base comment in
                arm9.ld for why this is effectively zero size in this game. */
            linker_addr(&raw const __arm7_wram_base)
        },
    }
}

/*(asm-corpus-filename: OS_SetArenaHi, objdump-symbol-name: arm9_bin/OS_SetArenaHi)DONE('os_arena')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A6D8")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_SetArenaHi(arena_index: u32, new_hi: u32) {
    raw_field!(SYS_AREA_ARENA_INFO_BUF as *mut OSArenaInfo, final mut hi[arena_index as usize]).set(new_hi);
}

/*(asm-corpus-filename: OS_SetArenaLo, objdump-symbol-name: arm9_bin/OS_SetArenaLo)DONE('os_arena')*/
#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A6EC")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_SetArenaLo(arena_index: u32, new_lo: u32) {
    raw_field!(SYS_AREA_ARENA_INFO_BUF as *mut OSArenaInfo, final mut lo[arena_index as usize]).set(new_lo);
}

#[unsafe(link_section = ".data.origaddr.020B2BAC")]
pub static OSi_ConsoleTypeCache: Global<u32> = Global::new(OSi_CONSOLE_NOT_DETECT);

#[unsafe(link_section = ".bss.origaddr.022B95E4")]
pub static OSi_IrqCallbackInfo: Global<OSIrqCallbackInfoTable> = Global::new(OSIrqCallbackInfoTable {
    dma_callbacks: [const {
        OSIrqCallbackInfo {
            func: None,
            enable: 0,
            arg: core::ptr::null_mut(),
        }
    }; REG_OS_IE_D3_SHIFT - REG_OS_IE_D0_SHIFT + 1],
    timer_callbacks: [const {
        OSIrqCallbackInfo {
            func: None,
            enable: 0,
            arg: core::ptr::null_mut(),
        }
    }; REG_OS_IE_T3_SHIFT - REG_OS_IE_T0_SHIFT + 1],
});

#[unsafe(link_section = ".bss.origaddr.022B98C4")]
pub static OSi_Initialized: Global<u32> = Global::new(0);

#[unsafe(link_section = ".bss.origaddr.022B98C8")]
pub static OSi_MAINEX_ENABLE: Global<u32> = Global::new(0); /* enable extended Main RAM 8 MiB (debug console only) */

/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */


/*cfg_select*/ } _
=> {} }

