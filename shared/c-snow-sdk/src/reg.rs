pub const REG_BASE: u32 = 0x04000000;

pub const REG_IME: *mut u16 = (REG_BASE + 0x208) as *mut u16;
pub const REG_IE: *mut u32 = (REG_BASE + 0x210) as *mut u32;
pub const REG_IF: *mut u32 = (REG_BASE + 0x214) as *mut u32;

