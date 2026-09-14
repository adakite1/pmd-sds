#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

use core::arch::asm;

const REG_BASE: u32 = 0x04000000;

#[inline(always)]
const fn reg_ime(reg_base: u32) -> *mut u32 {
    (reg_base + 0x208) as *mut u32
}

#[inline(always)]
fn reg_ime_off() {
    let mut v = REG_BASE;
    unsafe { asm!("", inout("ip") v); /* prevent llvm constant-folding */ }
    unsafe {
        core::intrinsics::volatile_store(reg_ime(v), v);
    }
}

unsafe extern "C" {
    static __arm7_wram_clear_start: u8;
    static __arm7_wram_clear_end: u8;
}

#[inline(always)]
fn linker_addr(symbol: *const u8) -> usize {
    symbol as usize
}

#[inline(always)]
fn clear_arm7_wram_range() {
    let start = linker_addr(&raw const __arm7_wram_clear_start);
    let end = linker_addr(&raw const __arm7_wram_clear_end);

    let mut current = start as *mut u32;
    while current < end as *mut u32 {
        unsafe {
            core::intrinsics::volatile_store(current, 0);
            current = current.add(1);
        }
    }
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn _start() -> ! {
    reg_ime_off();
    clear_arm7_wram_range();
    panic!();
}

