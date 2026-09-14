#![no_std]
#![no_main]

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
#[instruction_set(arm::a32)]
pub extern "C" fn _start() -> ! {
    loop {
        core::hint::spin_loop();
    }
}

