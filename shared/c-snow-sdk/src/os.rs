use crate::{Global, cp15_size_encoding, set_cp15_region};

cfg_select! {
armv5te => {

/* ====== ARM9 ====== */

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

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A3B4")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_InitArena() {
    if OSi_Initialized.get() != 0 {
        return;
    }
    OSi_Initialized.set(1);

    //TODO:
}

#[inline(never)]
#[unsafe(link_section = ".text.origaddr.0207A4B8")]
#[unsafe(no_mangle)]
pub extern "C" fn OS_InitArenaEx() {
    //TODO:

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
            //TODO:
        },
        1 => {
            /* no such arena */
            0
        },
        2 => {
            //TODO:
        },
        3 => {
            //TODO:
        },
        4 => {
            //TODO:
        },
        5 => {
            //TODO:
        },
        6 => {
            //TODO:
        },
    }
}
#[unsafe(link_section = ".data.origaddr.020B2BAC")]
pub static OSi_ConsoleTypeCache: Global<u32> = Global::new(OSi_CONSOLE_NOT_DETECT);

#[unsafe(link_section = ".bss.origaddr.022B98C4")]
pub static OSi_Initialized: Global<u32> = Global::new(0);

#[unsafe(link_section = ".bss.origaddr.022B98C8")]
pub static OSi_MAINEX_ENABLE: Global<u32> = Global::new(0); /* enable extended Main RAM 8 MiB (debug console only) */

/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */


/*cfg_select*/ } _
=> {} }

