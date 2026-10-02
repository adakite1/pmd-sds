//! This module is the one-stop-shop for absolutely everything regarding the 0x1000 size SDK and System Area at the top of Main RAM, including constants specifying addresses residing in this range.

pub mod red;

use crate::consts::OS_ARENA_MAX;

pub const SYS_AREA_START: u32 = 0x027FF000;

pub const SYS_AREA_ARENA_INFO_BUF: u32 = SYS_AREA_START + 0xDA0;

#[repr(C)]
pub struct OSArenaInfo {
    /// Arena live bound low
    pub lo: [u32; OS_ARENA_MAX],
    /// Arena live bound high
    pub hi: [u32; OS_ARENA_MAX],
}

/* MMAP.H */

pub const HW_CARD_ROM_HEADER_SIZE: u32 = 0x00000160;
pub const HW_DOWNLOAD_PARAMETER_SIZE: u32 = 0x00000020;

