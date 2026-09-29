//! This module is the one-stop-shop for absolutely everything regarding the 0x1000 size SDK and System Area at the top of Main RAM, including constants specifying addresses residing in this range.

pub mod red;

pub const SYS_AREA_START: u32 = 0x027FF000;

/* MMAP.H */

pub const HW_CARD_ROM_HEADER_SIZE: u32 = 0x00000160;
pub const HW_DOWNLOAD_PARAMETER_SIZE: u32 = 0x00000020;

