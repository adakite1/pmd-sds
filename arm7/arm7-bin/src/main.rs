#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

use core::arch::asm;

#[inline(always)]
fn linker_addr(symbol: *const u8) -> usize {
    symbol as usize
}

const REG_BASE: u32 = 0x04000000;

#[inline(always)]
const fn reg_ime(reg_base: u32) -> *mut u32 {
    (reg_base + 0x208) as *mut u32
}

#[inline(always)]
fn ime_off() {
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

unsafe extern "C" {
    static __svc_stack_top: u8;
    static __irq_stack_top: u8;
    static __irq_stack_size: u8;
}

const MODE_SVC: u32 = 19; //0x13
const MODE_IRQ: u32 = 18; //0x12
const MODE_SYS: u32 = 31; //0x1F

#[inline(always)]
fn init_banked_stacks() {
    unsafe {
        asm!(
            // 1. Supervisor (SVC) Mode
            "msr CPSR_c, {svc_mode}",
            "ldr sp, ={svc_top}",

            // 2. IRQ Mode
            "msr CPSR_c, {irq_mode}",
            "ldr r0, ={irq_top}",
            "mov sp, r0",
            "ldr r1, ={irq_size}",
            "sub r1, r0, r1",

            // 3. System (SYS) Mode
            "msr CPSR_fsxc, {sys_mode}",
            "sub sp, r1, #4",

            svc_mode = const MODE_SVC,
            irq_mode = const MODE_IRQ,
            sys_mode = const MODE_SYS,
            svc_top = sym __svc_stack_top,
            irq_top = sym __irq_stack_top,
            irq_size = sym __irq_stack_size,
            out("r0") _,
            out("r1") _,
        );
    }
    // Note: No, I will not use naked asm or global asm. This works, nothing spills to stack.
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn _start() -> ! {
    ime_off();
    clear_arm7_wram_range();
    init_banked_stacks();
    panic!();
}

