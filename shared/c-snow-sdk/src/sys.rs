//! This module is the one-stop-shop for absolutely everything regarding the 0x1000 size SDK and System Area at the top of Main RAM, including constants specifying addresses residing in this range.
//!
//! sys/dtcm_sys.rs also discusses the 0x40 region at the top of DTCM in-depth and is closely related since the SDK claims both very personally. Since the BIOS can conveniently find the DTCM area through
//! querying the subprocessor (CP15 c9) and DTCM is fast as well, it's a good place for the BIOS to reach the SDK and vice-versa.

pub mod dtcm_sys;

pub mod red;

use crate::consts::OS_ARENA_MAX;

pub const SYS_AREA_START: u32 = 0x027FF000;

pub const OS_ARENA_SYS_AREA_SHARED: u32 = 5;

pub const SYS_AREA_SYSTEM_WORK: u32 = SYS_AREA_START + 0xC00;

#[repr(C)]
pub struct OSArenaInfo {
    /// Arena live bound low
    pub lo: [u32; OS_ARENA_MAX],
    /// Arena live bound high
    pub hi: [u32; OS_ARENA_MAX],
}

#[repr(C)]
pub struct BootCheckInfo {
    pub _chipId1: u32,
    pub _chipId2: u32,
    pub _headerCrc: u16,
    pub _secureAreaCrc: u16,
    pub _crcMissingOrBad: u16,
    pub _secureAreaBad: u16,
    pub _HW_CHECK_DEBUGGER_SW: u16,
    pub _secureDisable: u16,
    pub _sioDebugExists: u16,
    pub _rtcStatus: u8,
    pub _random: u8,
    pub _unused: [u8; 8],
}
const _: () = assert!(core::mem::size_of::<BootCheckInfo>() == 32);

#[repr(C)]
pub struct OSSystemWork {
    pub _bootCheckInfo: BootCheckInfo,
    pub _resetParameter: u32,
    pub _padding5: [u8; 0x8],
    pub _romBaseOffset: u32,
    pub _cartridgeModuleInfo: [u8; 12],
    pub _vblankCount: u32,
    pub _wmBootBuf: [u8; 0x40],
    pub _nvramUserInfo: [u8; 0x100],
    pub _isd_reserved1: [u8; 0x1C],
    pub _HW_EXCP_VECTOR_MAIN: u32,
    pub arenaInfo: OSArenaInfo,
    pub _real_time_clock: [u8; 8],
    pub _dmaClearBuf: [u32; 4],
    pub _rom_header: [u8; 0x160],
    pub _isd_reserved2: [u8; 0x20],
    pub _pxiSignalParam: [u32; 2],
    pub pxiHandleChecker: [u32; 2],
    pub _mic_last_address: u32,
    pub _mic_sampling_data: u16,
    pub _wm_callback_control: u16,
    pub _wm_rssi_pool: u16,
    pub _set_ctrdg_module_info_once: u8,
    pub _is_ctrdg_exist: u8,
    pub _component_param: u32,
    pub _threadinfo_mainp: *mut core::ffi::c_void,
    pub _threadinfo_subp: *mut core::ffi::c_void,
    pub _button_XY: u16,
    pub _touch_panel: [u8; 4],
    pub _autoloadSync: u16,
    pub _lockIDFlag_mainp: [u32; 2],
    pub _lockIDFlag_subp: [u32; 2],
    pub _lock_VRAM_C: [u8; 8],
    pub _lock_VRAM_D: [u8; 8],
    pub _lock_WRAM_BLOCK0: [u8; 8],
    pub _lock_WRAM_BLOCK1: [u8; 8],
    pub _lock_CARD: [u8; 8],
    pub _lock_CARTRIDGE: [u8; 8],
    pub _lock_INIT: [u8; 8],
    pub _mmem_checker_mainp: u16,
    pub _mmem_checker_subp: u16,
    pub _padding4: [u8; 2],
    pub _command_area: u16,
}
const _: () = assert!(core::mem::size_of::<OSSystemWork>() == 0x400);
const _: () = assert!(core::mem::offset_of!(OSSystemWork, arenaInfo) == 0x1A0);
const _: () = assert!(core::mem::offset_of!(OSSystemWork, pxiHandleChecker) == 0x388);

/* MMAP.H */

pub const HW_CARD_ROM_HEADER_SIZE: u32 = 0x00000160;
pub const HW_DOWNLOAD_PARAMETER_SIZE: u32 = 0x00000020;

