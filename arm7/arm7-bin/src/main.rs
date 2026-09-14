#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

use core::arch::asm;

const REG_BASE: u32 = 0x04000000;

#[inline(always)]
#[instruction_set(arm::a32)]
const fn reg_ime(reg_base: u32) -> *mut u32 {
    (reg_base + 0x208) as *mut u32
}

#[inline(always)]
#[instruction_set(arm::a32)]
fn reg_ime_off() {
    let mut v = REG_BASE;
    unsafe { asm!("", inout("ip") v); /* prevent llvm constant-folding */ }
    unsafe {
        core::intrinsics::volatile_store(reg_ime(v), v);
    }
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
#[instruction_set(arm::a32)]
pub extern "C" fn _start() -> ! {
    reg_ime_off();
    panic!();
}

