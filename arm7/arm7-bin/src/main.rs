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

#[inline(always)]
fn copy_words_volatile(
    mut src: *const u32,
    mut dst: *mut u32,
    mut words: usize,
) {
    while words != 0 {
        unsafe {
            core::arch::asm!(
                "ldr {scratch}, [{src}], #4",
                "str {scratch}, [{dst}], #4",
                src = inout(reg) src,
                dst = inout(reg) dst,
                scratch = out(reg) _,
                options(nostack, preserves_flags),
            );
        }
        words -= 1;
    }
}

const HW_MAIN_MEM: usize = 0x02000000;
const HW_CARD_ROM_HEADER: usize = HW_MAIN_MEM + 0x007ffa80;
const HW_CARD_ROM_HEADER_SIZE: usize = 0x160;
const HW_DOWNLOAD_PARAMETER: usize = HW_CARD_ROM_HEADER + HW_CARD_ROM_HEADER_SIZE;
const HW_DOWNLOAD_PARAMETER_SIZE: usize = 0x20;

const IPL_CARD_ROM_HEADER: usize = 0x023fe940; /* IPL = "Initial Program Loader", which is the thing that runs before our code here even starts. These are magical source addresses that contain the data that needs to be copied. (I don't know who fills it initially, it's probably the IPL since this is data stored in the cartridge) */
const IPL_DOWNLOAD_PARAMETER: usize = 0x023fe904;

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn _start() -> ! {
    ime_off();
    clear_arm7_wram_range();
    init_banked_stacks();
    copy_words_volatile(
        IPL_CARD_ROM_HEADER as *const u32,
        HW_CARD_ROM_HEADER as *mut u32,
        HW_CARD_ROM_HEADER_SIZE / 4,
    );
    copy_words_volatile(
        IPL_DOWNLOAD_PARAMETER as *const u32,
        HW_DOWNLOAD_PARAMETER as *mut u32,
        HW_DOWNLOAD_PARAMETER_SIZE / 4,
    );
    panic!();
}

