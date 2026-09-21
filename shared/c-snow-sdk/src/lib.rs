#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

#[cfg(armv4t)]
#[unsafe(no_mangle)]
pub extern "C" fn HardwareInterrupt() -> ! {
    core::hint::black_box(2+2);
    panic!();
}

#[cfg(armv4t)]
#[unsafe(no_mangle)]
pub extern "C" fn SnowSpMain() -> ! {
    core::hint::black_box(1+1);
    panic!();
}

#[cfg(armv5te)]
#[unsafe(no_mangle)]
#[unsafe(link_section = ".itcm.text")]
pub extern "C" fn HardwareInterrupt() -> ! {
    core::hint::black_box(2+2);
    panic!();
}

#[cfg(armv5te)]
#[unsafe(no_mangle)]
pub extern "C" fn SnowMain() -> ! {
    core::hint::black_box(1+1);
    panic!();
}

