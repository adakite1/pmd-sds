#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

#[unsafe(no_mangle)]
pub extern "C" fn HardwareInterrupt() -> ! {
    core::hint::black_box(2+2);
    panic!();
}

#[unsafe(no_mangle)]
pub extern "C" fn SnowMain() -> ! {
    core::hint::black_box(1+1);
    panic!();
}

