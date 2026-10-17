use raw_traversal::{RegularRW, VolatileMut, VolatileRW};

/// Legacy ARM atomic swap
pub trait Swp<T> {
    /// Returns `old_val` in the register that held `val`
    fn swp(&self, val: T) -> T;
    /// Returns `(old_val, val)`
    fn swp2(&self, val: T) -> (T, T);
}

#[inline(always)]
#[instruction_set(arm::a32)]
fn swp_word(ptr: *mut u32, val: u32) -> u32 {
    let old_val: u32;
    unsafe {
        core::arch::asm!(
            "swp {val}, {val}, [{ptr}]",
            val = inout(reg) val => old_val,
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
    old_val
}

#[inline(always)]
#[instruction_set(arm::a32)]
fn swp2_word(ptr: *mut u32, val: u32) -> (u32, u32) {
    let old_val: u32;
    unsafe {
        core::arch::asm!(
            "swp {old_val}, {val}, [{ptr}]",
            old_val = out(reg) old_val, // not lateout because "If the same register is specified as <Rn> (ptr) and <Rm> (val), or <Rn> (ptr) and <Rd> (old_val), the result is UNPREDICTABLE" - DDI-0100I
            val = in(reg) val,
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
    (old_val, val)
}

#[inline(always)]
#[instruction_set(arm::a32)]
fn swp_byte(ptr: *mut u8, val: u8) -> u8 {
    let old_val: u32;
    unsafe {
        core::arch::asm!(
            "swpb {val}, {val}, [{ptr}]",
            val = inout(reg) val as u32 => old_val,
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
    old_val as u8
}

#[inline(always)]
#[instruction_set(arm::a32)]
fn swp2_byte(ptr: *mut u8, val: u8) -> (u8, u8) {
    let old_val: u32;
    unsafe {
        core::arch::asm!(
            "swpb {old_val}, {val}, [{ptr}]",
            old_val = out(reg) old_val,
            val = in(reg) val as u32,
            ptr = in(reg) ptr,
            options(nostack, preserves_flags),
        );
    }
    (old_val as u8, val)
}

macro_rules! impl_swp {
    ($($rw:ty),*) => {$(
        impl Swp<u32> for VolatileMut<u32, $rw> {
            #[inline(always)]
            fn swp(&self, val: u32) -> u32 {
                swp_word(self.as_ptr(), val)
            }

            #[inline(always)]
            fn swp2(&self, val: u32) -> (u32, u32) {
                swp2_word(self.as_ptr(), val)
            }
        }

        impl Swp<u8> for VolatileMut<u8, $rw> {
            #[inline(always)]
            fn swp(&self, val: u8) -> u8 {
                swp_byte(self.as_ptr(), val)
            }

            #[inline(always)]
            fn swp2(&self, val: u8) -> (u8, u8) {
                swp2_byte(self.as_ptr(), val)
            }
        }
    )*};
}

// Aligned rw types only
impl_swp!(VolatileRW, RegularRW);

