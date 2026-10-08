cfg_select! {
armv5te => {

/* ====== ARM9 ====== */


/*cfg_select*/ }
armv4t => {

/* ====== ARM7 ====== */

use core::ffi::c_void;

use crate::asm::{SVC_SoftReset_ASM, SVC_WaitByLoop_ASM, SVC_IntrWait_ASM, SVC_VBlankIntrWait_ASM, SVC_Halt_ASM, SVC_Sleep_ASM, SVC_SoundBias_ASM, SVC_Div_ASM, SVC_CpuSet_ASM, SVC_CpuFastSet_ASM, SVC_Sqrt_ASM, SVC_CRC16_ASM, SVC_IsMainMemoryExtended_ASM, SVC_BitUnPack_ASM, SVC_LZ77UnCompLdrReadWriteBytes_ASM, SVC_LZ77UnCompCbReadWriteHalfwords_ASM, SVC_HuffmanUnCompCbReadWriteWords_ASM, SVC_RLUnCompLdrReadWriteBytes_ASM, SVC_RLUnCompCbReadWriteHalfwords_ASM, SVC_QuarterSineLUTResolution64_ASM, SVC_OctMultFracLUTResolution768_ASM, SVC_VolumeLogToMulLinLUTResolution724_ASM};

/*(asm-corpus-filename: arm7/wram/sub_038037B4, objdump-symbol-name: arm7_bin/SVC_SoftReset)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037B4")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_SoftReset() {
    SVC_SoftReset_ASM()
}

/*(asm-corpus-filename: arm7/wram/sub_038037B8, objdump-symbol-name: arm7_bin/SVC_WaitByLoop)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037B8")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_WaitByLoop(count: i32) {
    SVC_WaitByLoop_ASM(count)
}

/*(asm-corpus-filename: arm7/wram/sub_038037BC, objdump-symbol-name: arm7_bin/SVC_IntrWait)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037BC")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_IntrWait(discard_old: u32, irqs_mask: u32) {
    SVC_IntrWait_ASM(discard_old, irqs_mask)
}

/*(asm-corpus-filename: arm7/wram/sub_038037CC, objdump-symbol-name: arm7_bin/SVC_VBlankIntrWait)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037CC")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_VBlankIntrWait() {
    SVC_VBlankIntrWait_ASM()
}

/*(asm-corpus-filename: arm7/wram/sub_038037D2, objdump-symbol-name: arm7_bin/SVC_Halt)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037D2")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_Halt() {
    SVC_Halt_ASM()
}

/*(asm-corpus-filename: arm7/wram/sub_038037D6, objdump-symbol-name: arm7_bin/SVC_Sleep)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037D6")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_Sleep() {
    SVC_Sleep_ASM()
}

/*(asm-corpus-filename: arm7/wram/sub_038037DA, objdump-symbol-name: arm7_bin/SVC_SoundBias)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037DA")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_SoundBias(direction: u32, delay: u32) {
    SVC_SoundBias_ASM(direction, delay)
}

/*(asm-corpus-filename: arm7/wram/sub_038037DE, objdump-symbol-name: arm7_bin/SVC_SoundBiasSet)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037DE")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_SoundBiasSet(delay: u32) {
    SVC_SoundBias_ASM(1, delay)
}

/*(asm-corpus-filename: arm7/wram/sub_038037E6, objdump-symbol-name: arm7_bin/SVC_SoundBiasReset)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037E6")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_SoundBiasReset(delay: u32) {
    SVC_SoundBias_ASM(0, delay)
}

/*(asm-corpus-filename: arm7/wram/sub_038037EE, objdump-symbol-name: arm7_bin/SVC_Div)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037EE")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_Div(numerator: i32, denominator: i32) -> i32 {
    SVC_Div_ASM(numerator, denominator).0
}

/*(asm-corpus-filename: arm7/wram/sub_038037F2, objdump-symbol-name: arm7_bin/SVC_Mod)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037F2")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_Mod(numerator: i32, denominator: i32) -> i32 {
    SVC_Div_ASM(numerator, denominator).1
}

/*(asm-corpus-filename: arm7/wram/sub_038037F8, objdump-symbol-name: arm7_bin/SVC_CpuSet)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037F8")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_CpuSet(src: *const c_void, dst: *mut c_void, control: u32) {
    SVC_CpuSet_ASM(src, dst, control);
}

/*(asm-corpus-filename: arm7/wram/sub_038037FC, objdump-symbol-name: arm7_bin/SVC_CpuFastSet)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.038037FC")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_CpuFastSet(src: *const c_void, dst: *mut c_void, control: u32) {
    SVC_CpuFastSet_ASM(src, dst, control);
}

/*(asm-corpus-filename: arm7/wram/sub_03803800, objdump-symbol-name: arm7_bin/SVC_Sqrt)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803800")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_Sqrt(value: u32) -> u32 {
    SVC_Sqrt_ASM(value)
}

/*(asm-corpus-filename: arm7/wram/sub_03803804, objdump-symbol-name: arm7_bin/SVC_CRC16)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803804")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_CRC16(seed: u32, data: *const c_void, len: u32) -> u32 {
    SVC_CRC16_ASM(seed, data, len).0
}

/*(asm-corpus-filename: arm7/wram/sub_03803808, objdump-symbol-name: arm7_bin/SVC_IsMainMemoryExtended)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803808")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_IsMainMemoryExtended() -> u32 {
    SVC_IsMainMemoryExtended_ASM()
}

/*(asm-corpus-filename: arm7/wram/sub_0380380C, objdump-symbol-name: arm7_bin/SVC_BitUnPack)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.0380380C")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_BitUnPack(src: *const c_void, dst: *mut c_void, info: *const c_void) {
    SVC_BitUnPack_ASM(src, dst, info)
}

/* name is about cb vs direct ldr from memory, the read used is `ldrb` */
/*(asm-corpus-filename: arm7/wram/sub_03803810, objdump-symbol-name: arm7_bin/SVC_LZ77UnCompLdrReadWriteBytes)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803810")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_LZ77UnCompLdrReadWriteBytes(src: *const c_void, dst: *mut c_void) {
    SVC_LZ77UnCompLdrReadWriteBytes_ASM(src, dst)
}

/*(asm-corpus-filename: arm7/wram/sub_03803814, objdump-symbol-name: arm7_bin/SVC_LZ77UnCompCbReadWriteHalfwords)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803814")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_LZ77UnCompCbReadWriteHalfwords(src: *const c_void, dst: *mut c_void, param: u32, callbacks: *const c_void) -> i32 {
    SVC_LZ77UnCompCbReadWriteHalfwords_ASM(src, dst, param, callbacks)
}

/*(asm-corpus-filename: arm7/wram/sub_03803818, objdump-symbol-name: arm7_bin/SVC_HuffmanUnCompCbReadWriteWords)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803818")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_HuffmanUnCompCbReadWriteWords(src: *const c_void, dst: *mut c_void, tree_buffer: *mut c_void, callbacks: *const c_void) -> i32 {
    SVC_HuffmanUnCompCbReadWriteWords_ASM(src, dst, tree_buffer, callbacks)
}

/* name is about cb vs direct ldr from memory, the read used is `ldrb` */
/*(asm-corpus-filename: arm7/wram/sub_0380381C, objdump-symbol-name: arm7_bin/SVC_RLUnCompLdrReadWriteBytes)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.0380381C")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_RLUnCompLdrReadWriteBytes(src: *const c_void, dst: *mut c_void) {
    SVC_RLUnCompLdrReadWriteBytes_ASM(src, dst)
}

/*(asm-corpus-filename: arm7/wram/sub_03803820, objdump-symbol-name: arm7_bin/SVC_RLUnCompCbReadWriteHalfwords)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803820")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_RLUnCompCbReadWriteHalfwords(src: *const c_void, dst: *mut c_void, param: u32, callbacks: *const c_void) -> i32 {
    SVC_RLUnCompCbReadWriteHalfwords_ASM(src, dst, param, callbacks)
}

/*(asm-corpus-filename: arm7/wram/sub_03803824, objdump-symbol-name: arm7_bin/SVC_QuarterSineLUTResolution64)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803824")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_QuarterSineLUTResolution64(index: u32) -> i32 {
    SVC_QuarterSineLUTResolution64_ASM(index)
}

/** Fractional part of octave multiplier as 16-bit values */
/*(asm-corpus-filename: arm7/wram/sub_03803828, objdump-symbol-name: arm7_bin/SVC_OctMultFracLUTResolution768)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.03803828")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_OctMultFracLUTResolution768(index: u32) -> u32 {
    SVC_OctMultFracLUTResolution768_ASM(index)
}

/// Log volume in ~0.1 dB steps, to `SOUNDxCNT bits 0..6` Mul:
///   10^( dB / 20 ) = 10^( ((i − 723) / 10) / 20 )
///   Sample multiplier = Mul(i)/128 × Div(i)
///  Mul(i) ≈* round( 128 * 10^((i − 723)/200) / Div(i) )
///  Div(i) = { ÷16 if 0 <= i <= 482, ÷4 if 483 <= i <= 602, ÷2 if 603 <= i <= 662, ÷1 if 663 <= i <= 723 }
///  * except for entries 602, 662, and 722, where they are 126 instead of 127, and the last entry, which is 127 instead of 128
/*(asm-corpus-filename: arm7/wram/sub_0380382C, objdump-symbol-name: arm7_bin/SVC_VolumeLogToMulLinLUTResolution724)DONE('svc')*/
#[inline(never)]
#[unsafe(link_section = ".text.wram.origaddr.0380382C")]
#[unsafe(no_mangle)]
#[instruction_set(arm::t32)]
pub extern "C" fn SVC_VolumeLogToMulLinLUTResolution724(index: u32) -> u32 {
    SVC_VolumeLogToMulLinLUTResolution724_ASM(index)
}

/*cfg_select*/ } _
=> {} }

