use core::ffi::c_void;

use crate::{consts::HW_PSR_IRQ_DISABLE, reg::REG_BASE};

/// Returns the original flag value of either 0 or [`HW_PSR_IRQ_DISABLE`],
/// to be handed back to [`OS_CpuRestoreIrq_ASM`] later.
#[inline(always)]
pub fn OS_CpuDisableIrq_ASM() -> u32 {
    let prev: u32;
    unsafe {
        core::arch::asm!(
            "mrs {prev}, cpsr",
            "orr {tmp}, {prev}, #{irq_disable}",
            "msr cpsr_c, {tmp}",
            "and {prev}, {prev}, #{irq_disable}",
            irq_disable = const HW_PSR_IRQ_DISABLE,
            prev = out(reg) prev,
            tmp = out(reg) _,
            options(nostack, preserves_flags),
        );
    }
    prev
}

#[inline(always)]
pub fn OS_CpuRestoreIrq_ASM(state: u32) -> u32 {
    let prev: u32;
    unsafe {
        core::arch::asm!(
            "mrs {cpsr}, cpsr",
            "bic {tmp}, {cpsr}, #{irq_disable}",
            "orr {tmp}, {tmp}, {state}",
            "msr cpsr_c, {tmp}",
            "and {state}, {cpsr}, #{irq_disable}",
            irq_disable = const HW_PSR_IRQ_DISABLE,
            state = inout(reg) state => prev,
            cpsr = out(reg) _,
            tmp = out(reg) _,
            options(nostack, preserves_flags),
        );
    }
    prev
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_SoftReset_ASM() {
    unsafe {
        core::arch::asm!(
            "svc #0",
            lateout("r0") _,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_WaitByLoop_ASM(count: i32) {
    unsafe {
        core::arch::asm!(
            "svc #3",
            inout("r0") count => _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_IntrWait_ASM(discard_old: u32, irqs_mask: u32) {
    unsafe {
        core::arch::asm!(
            "mov r12, r2",
            "movs r2, #0",
            "svc #4",
            inout("r0") discard_old => _,
            inout("r1") irqs_mask => _,
            inout("r2") REG_BASE => _,
            lateout("r3") _,
            lateout("r12") _,
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_VBlankIntrWait_ASM() {
    unsafe {
        core::arch::asm!(
            "svc #5",
            in("r2") 0u32,
            lateout("r0") _,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_Halt_ASM() {
    unsafe {
        core::arch::asm!(
            "svc #6",
            lateout("r0") _,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_Sleep_ASM() {
    unsafe {
        core::arch::asm!(
            "svc #7",
            lateout("r0") _,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_SoundBias_ASM(direction: u32, delay: u32) {
    unsafe {
        core::arch::asm!(
            "svc #8",
            inout("r0") direction => _,
            inout("r1") delay => _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_Div_ASM(numerator: i32, denominator: i32) -> (i32, i32, u32) {
    let quotient: i32;
    let remainder: i32;
    let abs_quotient: u32;
    unsafe {
        core::arch::asm!(
            "svc #9",
            inout("r0") numerator => quotient,
            inout("r1") denominator => remainder,
            lateout("r3") abs_quotient,
            options(preserves_flags),
        );
    }
    (quotient, remainder, abs_quotient)
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_CpuSet_ASM(src: *const c_void, dst: *mut c_void, control: u32) -> (*const c_void, *mut c_void) {
    let src_advanced_only_in_32bit_mode_because_16bit_mode_indexes_with_r5_and_also_32bit_fill_mode_advances_this_by_4bytes_only_while_copy_mode_advances_to_the_end_for_similar_reason_as_cpufastset: *const c_void;
    let dst_advanced_only_in_32bit_mode_because_16bit_mode_indexes_with_r5: *mut c_void;
    unsafe {
        core::arch::asm!(
            "svc #11",
            in("r2") control,
            inout("r0") src => src_advanced_only_in_32bit_mode_because_16bit_mode_indexes_with_r5_and_also_32bit_fill_mode_advances_this_by_4bytes_only_while_copy_mode_advances_to_the_end_for_similar_reason_as_cpufastset,
            inout("r1") dst => dst_advanced_only_in_32bit_mode_because_16bit_mode_indexes_with_r5,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    (src_advanced_only_in_32bit_mode_because_16bit_mode_indexes_with_r5_and_also_32bit_fill_mode_advances_this_by_4bytes_only_while_copy_mode_advances_to_the_end_for_similar_reason_as_cpufastset, dst_advanced_only_in_32bit_mode_because_16bit_mode_indexes_with_r5)
    // Unless the protection check aborts the call say if length is 0 or the start or (calculated) end of the source has bits 25-27 clear (like if you try to copy from below 0x02000000, then both come back unchanged
    // CpuSet + arm9 (only) + 32-bit fill mode = the fill value gets read before the first end check, so if you give a length of zero src_advanced is still advanced by only 4 bytes
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_CpuFastSet_ASM(src: *const c_void, dst: *mut c_void, control: u32) -> (*const c_void, *mut c_void) {
    let src_advanced_only_in_copy_mode_because_you_read_and_fill_with_that_one_value_in_the_other_mode: *const c_void;
    let dst_advanced: *mut c_void;
    unsafe {
        core::arch::asm!(
            "svc #12",
            in("r2") control,
            inout("r0") src => src_advanced_only_in_copy_mode_because_you_read_and_fill_with_that_one_value_in_the_other_mode,
            inout("r1") dst => dst_advanced,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    (src_advanced_only_in_copy_mode_because_you_read_and_fill_with_that_one_value_in_the_other_mode, dst_advanced)
    // Unless the protection check aborts the call say if length is 0 or the start or (calculated) end of the source has bits 25-27 clear (like if you try to copy from below 0x02000000, then both come back unchanged)
}

/// Find a power-of-two starting guess by something like bisection and then do Newton's method in a second loop.
#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_Sqrt_ASM(value: u32) -> u32 {
    let root: u32;
    unsafe {
        core::arch::asm!(
            "svc #13",
            inout("r0") value => root,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    root
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_CRC16_ASM(seed: u32, data: *const c_void, len: u32) -> (u32, *const c_void, u32) {
    let crc: u32;
    let data_end: *const c_void;
    let last_halfword: u32;
    unsafe {
        core::arch::asm!(
            "svc #14",
            in("r2") len,
            inout("r0") seed => crc,
            inout("r1") data => data_end,
            lateout("r3") last_halfword,
            options(preserves_flags),
        );
    }
    (crc, data_end, last_halfword)
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_IsMainMemoryExtended_ASM() -> u32 {
    let extended: u32;
    unsafe {
        core::arch::asm!(
            "svc #15",
            lateout("r0") extended,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    extended
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_BitUnPack_ASM(src: *const c_void, dst: *mut c_void, info: *const c_void) {
    unsafe {
        core::arch::asm!(
            "svc #16",
            in("r2") info,
            inout("r0") src => _,
            inout("r1") dst => _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

/* name is about cb vs direct ldr from memory, the read used is `ldrb` */
#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_LZ77UnCompLdrReadWriteBytes_ASM(src: *const c_void, dst: *mut c_void) {
    unsafe {
        core::arch::asm!(
            "svc #17",
            inout("r0") src => _,
            inout("r1") dst => _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_LZ77UnCompCbReadWriteHalfwords_ASM(src: *const c_void, dst: *mut c_void, param: u32, callbacks: *const c_void) -> i32 {
    let result: i32;
    unsafe {
        core::arch::asm!(
            "svc #18",
            in("r2") param,
            inout("r0") src => result,
            inout("r1") dst => _,
            inout("r3") callbacks => _,
            options(preserves_flags),
        );
    }
    result
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_HuffmanUnCompCbReadWriteWords_ASM(src: *const c_void, dst: *mut c_void, tree_buffer: *mut c_void, callbacks: *const c_void) -> i32 {
    let result: i32;
    unsafe {
        core::arch::asm!(
            "svc #19",
            in("r2") tree_buffer,
            inout("r0") src => result,
            inout("r1") dst => _,
            inout("r3") callbacks => _,
            options(preserves_flags),
        );
    }
    result
}

/* name is about cb vs direct ldr from memory, the read used is `ldrb` */
#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_RLUnCompLdrReadWriteBytes_ASM(src: *const c_void, dst: *mut c_void) {
    unsafe {
        core::arch::asm!(
            "svc #20",
            inout("r0") src => _,
            inout("r1") dst => _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_RLUnCompCbReadWriteHalfwords_ASM(src: *const c_void, dst: *mut c_void, param: u32, callbacks: *const c_void) -> i32 {
    let result: i32;
    unsafe {
        core::arch::asm!(
            "svc #21",
            in("r2") param,
            inout("r0") src => result,
            inout("r1") dst => _,
            inout("r3") callbacks => _,
            options(preserves_flags),
        );
    }
    result
}

#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_QuarterSineLUTResolution64_ASM(index: u32) -> i32 {
    let value: i32;
    unsafe {
        core::arch::asm!(
            "svc #26",
            inout("r0") index => value,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    value
}

/** Fractional part of octave multiplier as 16-bit values */
#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_OctMultFracLUTResolution768_ASM(index: u32) -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!(
            "svc #27",
            inout("r0") index => value,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    value
}

/// Log volume in ~0.1 dB steps, to `SOUNDxCNT bits 0..6` Mul:
///   10^( dB / 20 ) = 10^( ((i − 723) / 10) / 20 )
///   Sample multiplier = Mul(i)/128 × Div(i)
///  Mul(i) ≈* round( 128 * 10^((i − 723)/200) / Div(i) )
///  Div(i) = { ÷16 if 0 <= i <= 482, ÷4 if 483 <= i <= 602, ÷2 if 603 <= i <= 662, ÷1 if 663 <= i <= 723 }
///  * except for entries 602, 662, and 722, where they are 126 instead of 127, and the last entry, which is 127 instead of 128
#[inline(always)]
#[instruction_set(arm::t32)]
pub fn SVC_VolumeLogToMulLinLUTResolution724_ASM(index: u32) -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!(
            "svc #28",
            inout("r0") index => value,
            lateout("r1") _,
            lateout("r3") _,
            options(preserves_flags),
        );
    }
    value
}

