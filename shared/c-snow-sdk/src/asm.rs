use crate::consts::HW_PSR_IRQ_DISABLE;

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

