#![no_std]
#![no_main]

pub mod marks;
#[cfg(any(armv4t, armv5te))]
mod helpers;

pub mod prelude;

