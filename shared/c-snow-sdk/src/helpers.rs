use core::arch::naked_asm;

#[cfg(not(target_feature = "thumb-mode"))]
#[unsafe(no_mangle)]
#[cfg_attr(armv4t, unsafe(link_section = ".wram.text.__aeabi_uread4"))]
#[unsafe(naked)]
pub unsafe extern "C" fn __aeabi_uread4(address: *const u8) -> i32 {
    naked_asm!(
        "ldrb r1, [r0]",
        "ldrb r2, [r0, #1]",
        "ldrb r3, [r0, #2]",
        "ldrb r0, [r0, #3]",
        "orr r0, r3, r0, lsl #8",
        "orr r1, r1, r2, lsl #8",
        "orr r0, r1, r0, lsl #16",
        "bx lr",
    );
}

#[cfg(target_feature = "thumb-mode")]
#[unsafe(no_mangle)]
#[cfg_attr(armv4t, unsafe(link_section = ".wram.text.__aeabi_uread4"))]
#[unsafe(naked)]
pub unsafe extern "C" fn __aeabi_uread4(address: *const u8) -> i32 {
    naked_asm!(
        "ldrb r1, [r0]",
        "ldrb r2, [r0, #1]",
        "lsls r2, r2, #8",
        "adds r1, r2, r1",
        "ldrb r2, [r0, #2]",
        "lsls r2, r2, #16",
        "ldrb r0, [r0, #3]",
        "lsls r0, r0, #24",
        "adds r0, r0, r2",
        "adds r0, r0, r1",
        "bx lr",
    );
}

#[unsafe(no_mangle)]
#[cfg_attr(armv4t, unsafe(link_section = ".wram.text.__aeabi_uwrite4"))]
#[unsafe(naked)]
pub unsafe extern "C" fn __aeabi_uwrite4(value: i32, address: *mut u8) -> i32 {
    naked_asm!(
        "lsrs r2, r0, #24",
        "strb r0, [r1]",
        "strb r2, [r1, #3]",
        "lsrs r2, r0, #16",
        "strb r2, [r1, #2]",
        "lsrs r2, r0, #8",
        "strb r2, [r1, #1]",
        "bx lr",
    );
}

#[cfg(not(target_feature = "thumb-mode"))]
#[unsafe(no_mangle)]
#[cfg_attr(armv4t, unsafe(link_section = ".wram.text.__aeabi_uread8"))]
#[unsafe(naked)]
pub unsafe extern "C" fn __aeabi_uread8(address: *const u8) -> i64 {
    naked_asm!(
        "ldrb r12, [r0]",
        "ldrb r2, [r0, #1]",
        "ldrb r3, [r0, #2]",
        "ldrb r1, [r0, #3]",
        "orr r1, r3, r1, lsl #8",
        "orr r2, r12, r2, lsl #8",
        "orr r2, r2, r1, lsl #16",
        "ldrb r1, [r0, #5]",
        "ldrb r3, [r0, #4]!",
        "orr r1, r3, r1, lsl #8",
        "ldrb r3, [r0, #2]",
        "ldrb r0, [r0, #3]",
        "orr r0, r3, r0, lsl #8",
        "orr r1, r1, r0, lsl #16",
        "mov r0, r2",
        "bx lr",
    );
}

#[cfg(target_feature = "thumb-mode")]
#[unsafe(no_mangle)]
#[cfg_attr(armv4t, unsafe(link_section = ".wram.text.__aeabi_uread8"))]
#[unsafe(naked)]
pub unsafe extern "C" fn __aeabi_uread8(address: *const u8) -> i64 {
    naked_asm!(
        "ldrb r1, [r0]",
        "ldrb r2, [r0, #1]",
        "lsls r2, r2, #8",
        "adds r1, r2, r1",
        "ldrb r2, [r0, #2]",
        "lsls r2, r2, #16",
        "ldrb r3, [r0, #3]",
        "lsls r3, r3, #24",
        "adds r2, r3, r2",
        "adds r2, r2, r1",
        "ldrb r1, [r0, #4]",
        "ldrb r3, [r0, #5]",
        "lsls r3, r3, #8",
        "adds r1, r3, r1",
        "ldrb r3, [r0, #6]",
        "lsls r3, r3, #16",
        "ldrb r0, [r0, #7]",
        "lsls r0, r0, #24",
        "adds r0, r0, r3",
        "adds r1, r0, r1",
        "movs r0, r2",
        "bx lr",
    );
}

#[unsafe(no_mangle)]
#[cfg_attr(armv4t, unsafe(link_section = ".wram.text.__aeabi_uwrite8"))]
#[unsafe(naked)]
pub unsafe extern "C" fn __aeabi_uwrite8(value: i64, address: *mut u8) -> i64 {
    naked_asm!(
        "strb r0, [r2, #0]",
        "lsrs r3, r0, #8",
        "strb r3, [r2, #1]",
        "lsrs r3, r0, #16",
        "strb r3, [r2, #2]",
        "lsrs r3, r0, #24",
        "strb r3, [r2, #3]",
        "strb r1, [r2, #4]",
        "lsrs r3, r1, #8",
        "strb r3, [r2, #5]",
        "lsrs r3, r1, #16",
        "strb r3, [r2, #6]",
        "lsrs r3, r1, #24",
        "strb r3, [r2, #7]",
        "bx lr",
    );
}

