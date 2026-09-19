#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

use core::arch::asm;

use c_snow_sdk::{HardwareInterrupt, SnowSpMain};

#[inline(always)]
fn linker_addr(symbol: *const u8) -> u32 {
    symbol as u32
}

unsafe extern "C" {
    static __reg_base: u8;
}

#[inline(always)]
const fn reg_ime(reg_base: u32) -> *mut u32 {
    (reg_base + 0x208) as *mut u32
}

#[inline(always)]
fn ime_off() {
    let mut v = linker_addr(&raw const __reg_base);
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

#[inline(always)]
fn copy_range_volatile(
    mut src_start: *const u32,
    src_end: *const u32,
    mut dst: *mut u32,
) -> *mut u32 {
    while src_start < src_end {
        unsafe {
            core::arch::asm!(
                "ldr {scratch}, [{src}], #4",
                "str {scratch}, [{dst}], #4",
                src = inout(reg) src_start,
                dst = inout(reg) dst,
                scratch = out(reg) _,
                options(nostack, preserves_flags),
            );
        }
    }
    dst
}

#[inline(always)]
fn zero_fill_volatile(mut dst: *mut u32, mut size: usize) -> *mut u32 {
    debug_assert_eq!(size % core::mem::size_of::<u32>(), 0);
    while size != 0 {
        unsafe {
            core::arch::asm!(
                "str {zero}, [{dst}], #4",
                dst = inout(reg) dst,
                zero = in(reg) 0u32,
                options(nostack, preserves_flags),
            );
        }
        size -= 4;
    }
    dst
}

unsafe extern "C" {
    static __module_params: u8;
}

#[repr(C)]
struct ModuleParams {
    autoload_infos_start: *const u32,
    autoload_infos_end: *const u32,
    autoload_blocks: *const u32,
    bss_start: *const u32,
    bss_end: *const u32,
}

#[repr(C)]
struct AutoloadEntry {
    dest_addr: *mut u32,
    stored_size: u32,
    bss_size: u32,
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn do_autoload() -> ! {
    let module_params = unsafe { &*(linker_addr(&raw const __module_params) as *const ModuleParams) };
    let &ModuleParams { autoload_infos_start: mut current_entry_ptr, autoload_infos_end, autoload_blocks: mut current_src_start, .. } = module_params;
    unsafe { asm!("", inout("r1") current_entry_ptr); }
    unsafe { asm!("", in("r2") autoload_infos_end); }
    unsafe { asm!("", inout("r3") current_src_start); }

    while current_entry_ptr != autoload_infos_end {
        let autoload_entry = unsafe { &*(current_entry_ptr as *const AutoloadEntry) };
        let &AutoloadEntry { dest_addr, stored_size, bss_size } = autoload_entry;
        unsafe { asm!("", in("r0") dest_addr); }
        unsafe { asm!("", in("r4") stored_size); }
        unsafe { asm!("", in("r5") bss_size); }

        let current_src_end = unsafe { current_src_start.byte_add(stored_size as usize) };
        unsafe { asm!("", in("r8") current_src_end); }
        let dest_addr = copy_range_volatile(
            current_src_start,
            current_src_end,
            dest_addr,
        );

        current_src_start = current_src_end;

        unsafe {
            // bss
            zero_fill_volatile(dest_addr, bss_size as usize);

            current_entry_ptr = current_entry_ptr.add(3);
        }
    }

    unsafe {
        asm!(
            "bx lr",
            options(noreturn, nostack),
        );
    }
}

#[inline(always)]
fn zero_fill_range_volatile(
    mut dst_start: *mut u32,
    dst_end: *mut u32,
) -> *mut u32 {
    debug_assert_eq!((dst_end as u32 - dst_start as u32) as usize % core::mem::size_of::<u32>(), 0);
    while dst_start < dst_end {
        unsafe {
            core::arch::asm!(
                "str {zero}, [{dst}], #4",
                dst = inout(reg) dst_start,
                zero = in(reg) 0u32,
                options(nostack, preserves_flags),
            );
        }
    }
    dst_start
}

#[inline(always)]
fn clear_module_bss() {
    let module_params = unsafe { &*(linker_addr(&raw const __module_params) as *const ModuleParams) };
    let &ModuleParams { bss_start, bss_end, .. } = module_params;
    unsafe { asm!("", in("r1") bss_start); }
    unsafe { asm!("", in("r2") bss_end); }
    // bss
    zero_fill_range_volatile(bss_start as *mut u32, bss_end as *mut u32);
}

const HW_MTEST_MRAMDTCTDSZ_FLAG: usize = 0x027ffffa; /* *mut u16 */

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn test_memory() -> ! {
    let mut result = 1;

    let upper = HW_MTEST_MRAMDTCTDSZ_FLAG as *mut u16;
    let lower = (HW_MTEST_MRAMDTCTDSZ_FLAG - 0x400000) as *const u16;
    unsafe { asm!("", in("r2") upper); }
    unsafe { asm!("", in("r3") lower); }

    for test_value in 0u16..2 {
        // unsafe { asm!("", in("r1") test_value); }

        unsafe {
            core::intrinsics::volatile_store(upper, test_value);

            if test_value != core::intrinsics::volatile_load(lower) {
                result = 2;
                break;
            }
        }
    }
    
    unsafe { asm!("", in("r0") result); }

    unsafe {
        core::intrinsics::volatile_store(upper, result); /* HW_MTEST_MRAMDTCTDSZ_FLAG will be 2 if the lower and upper addresses did not alias, and 1 if they did alias, suggesting a smaller Main RAM size. On a normal 4MB Main RAM Nintendo DS, this is 1. */
    }

    unsafe {
        asm!(
            "bx lr",
            options(noreturn, nostack),
        );
    }
}

unsafe extern "C" {
    static __irq_handler_pointer: u8;
}

const NOT_EXCEPTION_VEC_TABLE_UNDEF_MEM: usize = 0xffff0000;

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
    unsafe {
        asm!(
            "bl {}",
            sym do_autoload,
            /* do_autoload uses some callee-saved non-volatile registers (r4-r11),
                which need to be preserved usually by the called function
                if it wants to use them, but since here we are using a
                manual `bx lr` to return, LLVM does not know that, so
                we need to tell LLVM which registers will be clobbered. */
            out("r4") _,
            out("r5") _,
            out("r8") _,
            clobber_abi("C"),
            options(nostack),
        );
    }
    clear_module_bss();
    unsafe {
        asm!(
            "bl {}",
            sym test_memory,
            clobber_abi("C"),
            options(nostack),
        );
    }
    unsafe {
        core::intrinsics::volatile_store(
            linker_addr(&raw const __irq_handler_pointer) as *mut u32,
            HardwareInterrupt as *const () as u32,
        ); /* set irq handler */
    }
    unsafe {
        asm!(
            "ldr r1, ={main}",
            "ldr lr, ={a}",
            /* high-vector mode is not set on the ARM7TDMI processor here,
                and GBATEK says the BIOS is not mirrored either, so 0xffff0000
                is undefined memory. So unlike the ARM9, here in ARM7, we just
                hang on undefined memory if we return here. */
            "bx r1",
            a = const NOT_EXCEPTION_VEC_TABLE_UNDEF_MEM,
            main = sym SnowSpMain,
            options(noreturn, nostack),
        );
    }
}

