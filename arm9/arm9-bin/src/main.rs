#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]

const _: () = assert!(
    (cfg!(feature = "na") as u8
        + cfg!(feature = "eu") as u8
        + cfg!(feature = "jp") as u8)
        == 1,
    "at least one of and no more than one of na/eu/jp must be enabled"
);

use core::arch::{asm, global_asm};

global_asm!(include_str!("secure_area.s"));

#[inline(always)]
fn linker_addr(symbol: *const u8) -> u32 {
    symbol as u32
}

unsafe extern "C" {
    static __reg_base: u8;
    static __palette_ram_base: u8;
    static __vram_base: u8;
    static __oam_base: u8;
}

#[inline(always)]
const fn reg_vcount(reg_base: u32) -> *const u16 {
    (reg_base + 0x6) as *const u16
}

#[inline(always)]
const fn reg_ime(reg_base: u32) -> *mut u32 {
    (reg_base + 0x208) as *mut u32
}

#[inline(always)]
fn ime_off(reg_base: u32) {
    unsafe {
        core::intrinsics::volatile_store(reg_ime(reg_base), reg_base);
    }
}

#[inline(always)]
fn wait_scanline_0(reg_base: u32) {
    while unsafe {
        core::intrinsics::volatile_load(reg_vcount(reg_base))
    } != 0 {
        core::hint::spin_loop();
    }
}

#[inline(always)]
pub const fn cp15_size_encoding(size: u32) -> u32 {
    assert!(size >= 0x1000);
    assert!(size.is_power_of_two());

    (size.trailing_zeros() - 1) << 1  /* PMSAv5 size encoding */
}

macro_rules! set_cp15_region {
    (
        region = $region:ident,
        base = $base:expr,
        size_encoding = $size:expr,
        enabled = $enabled:expr $(,)?
    ) => {{
        const VALUE: u32 = {
            let base = $base as u32;
            let size_raw = ($size as u32) & 0x3e;

            let exponent = (size_raw >> 1) + 1;
            let size_bytes = 1u32 << exponent;

            assert!(
                (base & 0x00000fff) == 0, /* Base cannot leak into bits [11:0] */
                "MPU base address must have bits [11:0] cleared (minimum 4KB alignment)"
            );

            assert!(
                (base & (size_bytes - 1)) == 0, /* Base must be naturally aligned to region size */
                "MPU base address is not naturally aligned to the region size"
            );

            (base & 0xfffff000)
                | size_raw
                | (if $enabled { 1 } else { 0 })
        };

        /* Configure an ARM Memory Protection Unit (MPU) managed memory region on an
            ARMv4/ARMv5 PMSA core (like the ARM946E-S on Nintendo DS)
            CRn = c6 contains MPU Region Base and Size Registers
            
            The value is:
            Bits [31:12] (Base Address): The physical starting address of the memory region
            Bits [11:6] (Reserved): SBZ
            Bits [5:1] (Size): The region size encoded as an exponent, defined by: Size = 2^(N+1) bytes where N is the 5-bit binary value with no shifting
            Bit [0] (Enable)
            
            This doesn't do much by itself, it just defines the regions. Later, we can start flipping:
            CRn = c2 (Cacheability), where Bit N sets whether reads and writes in Region N can allocate into the L1 I-Cache or D-Cache,
            CRn = c3 (Bufferability), where Bit N sets whether writes to Region N can use the hardware write buffer,
            CRn = c5 (Access Permissions), where each region has 2-bits of permissions representing No Access, Read-Only, and Full-Access for Privileged vs User modes
            and then finally flipping SCTLR Bit 0 (the M bit) enables all of them.
            
            Some more notes:
            - For any address, if it is present in multiple regions (overlapping regions),
                the highest-numbered region's configuration wins. So Region 7 wins over Region 0.
            - If a memory access touches anything unconfigured or permissions are violated,
                immediate Data Abort or Prefetch Abort exception.
            - The Cache Controller checks c2 and c3 bits to decide whether an access goes straight
                to memory or uses the cache and write buffers respectively. */
        core::arch::asm!(
            concat!("mcr p15, 0, {value}, c6, ", stringify!($region), ", 0"),
            value = in(reg) VALUE,
            options(nostack, preserves_flags)
        );
    }};
}

unsafe extern "C" {
    static __dtcm_base: u8;
}

pub struct RegionMask(pub u8);

impl RegionMask {
    pub const fn new() -> Self {
        Self(0)
    }

    pub const fn with(mut self, region: usize) -> Self {
        assert!(region < 8, "PMSAv5 only supports regions 0-7");
        self.0 |= 1 << region;
        self
    }

    pub const fn as_u32(self) -> u32 {
        self.0 as u32
    }
}

/* Region 1 (Main RAM) and Region 6 (ARM9 BIOS High Vectors) are both cacheable,
    everything else, like MMIO registers, the cartridge slots, and others remain uncached
    to prevent stale reads and timing hazards. */

pub const I_CACHE_REGIONS: u32 = RegionMask::new()
    .with(1)
    .with(6)
    .as_u32(); // 0x42 (66)

pub const D_CACHE_REGIONS: u32 = RegionMask::new()
    .with(1)
    .with(6)
    .as_u32(); // 0x42 (66)

/* Region 1 only (Write-Back for RAM) for bufferability, because the 0xffff0000 region
    maps to the ARM9 BIOS chip which is hard-wired mask ROM etched into the silicon at the factory
    so it is read-only. Also, the exception handlers live here and those shouldn't be overwritten anyways. */

pub const BUFFERABLE_REGIONS: u32 = RegionMask::new()
    .with(1)
    .as_u32(); // 0x02 (2)

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum AccessPermission {
    /// No access by anyone (when S = 0, R = 0).
    /// 
    /// In addition:
    /// - When S = 1, R = 0, read-only in privileged modes, not accessible in user mode.
    /// - When S = 0, R = 1, read-only in privileged modes, read-only in user mode.
    /// - When S = 1, R = 1, illegal state, reserved and unpredictable.
    NoAccess       = 0b00,
    /// Read/write in privileged modes, not accessible in user mode.
    PrivRwUserNone = 0b01,
    /// Read/write in privileged modes, read-only in user mode.
    PrivRwUserRo   = 0b10,
    /// Read/write by all modes.
    FullAccess     = 0b11,
}

pub const fn build_c5_mask(perms: [AccessPermission; 8]) -> u32 {
    let mut mask = 0u32;
    let mut i = 0;
    while i < 8 {
        mask |= (perms[i] as u32) << (i * 2);
        i += 1;
    }
    mask
}

#[repr(u32)]
#[derive(Copy, Clone, Debug)]
pub enum AccessPermissionExtended {
    /// No access by anyone.
    NoAccess       = 0b0000,
    /// Read/write in privileged modes, not accessible in user mode.
    PrivRwUserNone = 0b0001,
    /// Read/write in privileged modes, read-only in user mode.
    PrivRwUserRo   = 0b0010,
    /// Read/write by all modes.
    FullAccess     = 0b0011,
    // 0b0100 is UNP (UNPREDICTABLE).
    /// Read-only in privileged modes, not accessible in user mode.
    PrivRoUserNone = 0b0101,
    /// Read-only in privileged modes, read-only in user mode.
    PrivRoUserRo = 0b0110,
    // 0b0111 and all 0b1xxx are UNP.
}

pub const fn build_c5_mask_extended(perms: [AccessPermissionExtended; 8]) -> u32 {
    let mut mask = 0u32;
    let mut i = 0;
    while i < 8 {
        mask |= (perms[i] as u32) << (i * 4);
        i += 1;
    }
    mask
}

// 0x05100011
pub const IAP_CONFIG: u32 = build_c5_mask_extended([
    AccessPermissionExtended::PrivRwUserNone, // Region 0 (04000000-07FFFFFF): The broad ARM9 I/O, palettes, VRAM, OAM window
    /* 0x04000000  ARM9 I/O registers
        0x05000000  Palette RAM
        0x06000000  VRAM
        0x07000000  OAM */
    AccessPermissionExtended::PrivRwUserNone, // Region 1 (02000000-027FFFFF): Main RAM (including the 02400000-027FFFFF mirror on a normal 4 MiB Main RAM NDS)
    AccessPermissionExtended::NoAccess,       // Region 2 (027E0000-027FFFFF): Guard area around DTCM and c7 overlays (note that GBATEK's default is 027C0000-027FFFFF, which is a bit different)
    /* 0x027E0000..0x027E3FFF  c4 DTCM
        0x027E4000..0x027FEFFF  c2 remainder <== only this region is guarded since the other two parts have higher-numbered MPU regions describing it
        0x027FF000..0x027FFFFF  c7 Debug Console Work Area */
    AccessPermissionExtended::NoAccess,       // Region 3 (08000000-0FFFFFFF): GBA Slot (Execute-Never) (GBA cartridge ROM begins at 08000000, with max 32 MiB. GBA cartridge RAM is at 0A000000, max 64 KiB.)
    AccessPermissionExtended::NoAccess,       // Region 4 (027E0000-027E3FFF): DTCM (Execute-Never) (Higher MPU priority than c1 and c2)
    AccessPermissionExtended::PrivRwUserNone, // Region 5 (01000000-01FFFFFF): Describes the MPU permissions for the ITCM mirrors between range 0x01000000..0x01FFFFFF (upper 16 MiB of the full 32 MiB ITCM virtual window) (note that the linker script puts the ITCM image inside this range, so these permissions apply by default)
    AccessPermissionExtended::PrivRoUserNone, // Region 6 (FFFF0000-FFFF7FFF): BIOS ROM (Fetch only) (Only about ~3 KiB used according to GBATEK, exception vectors at 0xffff0000)
    AccessPermissionExtended::NoAccess,       // Region 7 (027FF000-027FFFFF): Debug Console Work Area (Execute-Never)
    /* Considering that r7 would only have its own real special memory region on an 8 MiB Main RAM debug-console,
        this is likely a debug-console and debugger work area. You can't run code from it, privileged data
        access is read-and-write, and both caching and write buffering is disables, which is exactly what
        you'd want since you want the debugger to observe the same contents in its work area as the CPU at all times (you want to get live data, and since you're in development phase the performance benefit of caching does not matter).

        GBATEK notes this as Shared Work instead but this is *NOT* the same as the one controlled by WRAMCNT, which
        can be 0 KiB, 16 KiB, or 32 KiB, and goes from 0x03000000 to 0x03FFFFFF.
        0x03000000..0x03007FFF  physical Shared WRAM
        0x03008000..0x0300FFFF  same 32 KiB again
        ...
        0x037F8000..0x037FFFFF  same 32 KiB again (this particular one is the usual Shared-WRAM alias)
        If 0 KiB is allocated to ARM9, according to GBATEK the ARM9 Shared-WRAM area is undefined. */
]);
const _: () = assert!(IAP_CONFIG == 0x05100011);

// 0x15111011
pub const DAP_CONFIG: u32 = build_c5_mask_extended([
    AccessPermissionExtended::PrivRwUserNone, // Region 0 (04000000-07FFFFFF): The broad ARM9 I/O, palettes, VRAM, OAM window
    /* 0x04000000  ARM9 I/O registers
        0x05000000  Palette RAM
        0x06000000  VRAM
        0x07000000  OAM */
    AccessPermissionExtended::PrivRwUserNone, // Region 1 (02000000-027FFFFF): Main RAM (including the 02400000-027FFFFF mirror on a normal 4 MiB Main RAM NDS)
    AccessPermissionExtended::NoAccess,       // Region 2 (027E0000-027FFFFF): Guard area around DTCM and c7 overlays (note that GBATEK's default is 027C0000-027FFFFF, which is a bit different)
    /* 0x027E0000..0x027E3FFF  c4 DTCM
        0x027E4000..0x027FEFFF  c2 remainder <== only this region is guarded since the other two parts have higher-numbered MPU regions describing it
        0x027FF000..0x027FFFFF  c7 Debug Console Work Area */
    AccessPermissionExtended::PrivRwUserNone, // Region 3 (08000000-0FFFFFFF): GBA Slot (RW) (GBA cartridge ROM begins at 08000000, with max 32 MiB. GBA cartridge RAM is at 0A000000, max 64 KiB.)
    AccessPermissionExtended::PrivRwUserNone, // Region 4 (027E0000-027E3FFF): DTCM (Execute-Never) (Higher MPU priority than c1 and c2)
    AccessPermissionExtended::PrivRwUserNone, // Region 5 (01000000-01FFFFFF): Describes the MPU permissions for the ITCM mirrors between range 0x01000000..0x01FFFFFF (upper 16 MiB of the full 32 MiB ITCM virtual window) (note that the linker script puts the ITCM image inside this range, so these permissions apply by default)
    AccessPermissionExtended::PrivRoUserNone, // Region 6 (FFFF0000-FFFF7FFF): BIOS ROM (Write-Protected)
    AccessPermissionExtended::PrivRwUserNone, // Region 7 (027FF000-027FFFFF): Debug Console Work Area (RW)
]);
const _: () = assert!(DAP_CONFIG == 0x15111011);

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn init_cp15() -> ! {
    unsafe {
        let mut sctlr: u32;

        /* System Control Register (SCTLR), system register identified by
            CRn (Primary Register) = c1 is the group designated for System Control Registers,
            CRm (Secondary Register) = c0,
            opcode2 = 0 */
        asm!("mrc p15, 0, {0}, c1, c0, 0", out(reg) sctlr, options(nomem, nostack)); /* nomem = these operations do not access general memory */

        /* Clear bits 0x000f9005 (bits 0, 2, 12, 15, 16, 17, 18, 19)
            bit 0   M   Protection unit (MMU) enable
            bit 1   A   Alignment fault check (1 will fault on unaligned access) (Since in ARMv5 ARM946E-S, unaligned 32-bit data reads don't abort, but instead load the aligned word and circularly rotate the byte into place, there is no abort generator for alignment at all, meaning this SBZ here)
            bit 2   C   DCache (data-cache) enable
            bit 3   W   Write buffer enable (On ARM946E-S this is set per-region so this global one SBO, according the manual)
            bit 4   P   32-bit Program space enable (reserved SBO, should be 1; for the transition from 26-bit addressing (ARMv2, ARMv3) to 32-bit addressing that was done by the time ARM946E-S became a thing)
            bit 5   D   32-bit Data space enable (reserved SBO, should be 1; for the transition from 26-bit addressing (ARMv2, ARMv3) to 32-bit addressing that was done by the time ARM946E-S became a thing)
            bit 6   L   Late Abort model select (reserved SBO, should be 1; for the transition from 26-bit addressing (ARMv2, ARMv3) to 32-bit addressing that was done by the time ARM946E-S became a thing)
            bit 7   B   Big-endian
            bit 8   S   System protection bit (SBZ)
            bit 9   R   ROM protection bit (SBZ)
            ...reserved SBZ
            bit 12  I   ICache (instruction-cache) enable
            bit 13  V   Alternate vector select (High Vectors, Exception vectors mapped to 0xffff0000)
            bit 14  RR  Round-robin replacement (Cache replacement strategy)
            bit 15  L4  Configure disable loading TBIT (with this enabled, data loads into the PC no longer checks the TBIT to switch to Thumb, but you can still switch modes with explicit bx and blx)
            bit 16  DT  Data RAM (DTCM) enable
            bit 17  DL  Data RAM load mode (enable. Since DTCM is layered *over* existing memory, a section of existing memory is basically inaccessible depending on whether anything was mapped there by the External Bus. Turning this on goes around DTCM and lets accesses to the DTCM address space *by the Data Bus (D-Side)* to fall through to the covered External Bus (Instruction Fetch targeting DTCM range goes through External Bus no matter what, the I-side does not have the physical wiring to access DTCM SRAM). Disabled when running real code since it defeats the point of fast DTCM)
            bit 18  IT  Instruction RAM (ITCM) enable
            bit 19  IL  Instruction RAM load mode (enable. Since ITCM is layered *over* existing memory, a section of existing memory is basically inaccessible depending on whether anything was mapped there by the External Bus. Turning this on goes around ITCM and lets accesses to the ITCM address space *by the Instruction Bus (I-Side)* to fall through to the covered External Bus (Data Access by the D-side like ldr and str *still gets routed to the ITCM SRAM*). Disabled when running real code since it defeats the point of fast ITCM)
            ...reserved SBZ */
        sctlr &= !0x000f9005;
        
        asm!("mcr p15, 0, {0}, c1, c0, 0", in(reg) sctlr, options(nomem, nostack)); /* Write back into SCTLR */

        asm!(
            /* CRn = c7 is the group designated for Cache, Branch Predictor, and Memory Barrier operations
                CRm = c5 targets I-Cache */
            "mcr p15, 0, {0}, c7, c5, 0",  // Invalidate instruction cache (ICIALLU)
            /* CPU register operand (r0) is defined as "Should Be Zero" (SBZ), and
                SBZ means software must write a value of 0 to this register to ensure compatibility with future hardware upgrades */
            /*  CRm = c6 targets D-Cache */
            "mcr p15, 0, {0}, c7, c6, 0",  // Invalidate data cache
            /*  CRm = c10 groups cache clean operations and pipeline/bus stall commands */
            "mcr p15, 0, {0}, c7, c10, 4", // Drain Write Buffer (CP15DSB)
            in(reg) 0,
            options(nostack),
        );

        set_cp15_region!(
            region = c0,
            base = 0x04000000,
            /* 0x04000000, 64 MiB */
            size_encoding = cp15_size_encoding(64 * 1024 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c1,
            base = 0x02000000,
            /* 0x02000000, 8 MiB */
            size_encoding = cp15_size_encoding(8 * 1024 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c2,
            base = 0x027E0000,
            /* 0x027E0000, 128 KiB */
            size_encoding = cp15_size_encoding(128 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c3,
            base = 0x08000000,
            /* 0x08000000, 128 MiB */
            size_encoding = cp15_size_encoding(128 * 1024 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c4,
            base = 0x027E0000,
            /* 0x027E0000, 16 KiB */
            size_encoding = cp15_size_encoding(16 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c5,
            base = 0x01000000,
            /* 0x01000000, 16 MiB */
            size_encoding = cp15_size_encoding(16 * 1024 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c6,
            base = 0xFFFF0000,
            /* 0xFFFF0000, 32 KiB */
            size_encoding = cp15_size_encoding(32 * 1024),
            enabled = true,
        );

        set_cp15_region!(
            region = c7,
            base = 0x027FF000,
            /* 0x027FF000, 4 KiB */
            size_encoding = cp15_size_encoding(4 * 1024),
            enabled = true,
        );

        let dtcm_base = linker_addr(&raw const __dtcm_base);
        asm!(
            /* CRn = c9 is designated for registers configuring the TCM
                ITCM: fast instruction memory at 0x00000000
                DTCM: fast data/stack memory at 0x027e0000 */
            "mcr p15, 0, {itcm}, c9, c1, 1", // ITCM Control Register
            "mcr p15, 0, {dtcm}, c9, c1, 0", // DTCM Control Register
            itcm = in(reg) 0x20,
            dtcm = in(reg) (dtcm_base | 0x0000000a),
            /* Bits 1-5 of the lower part (0x20, 0x0a) means virtual size,
                defined as 512 << N where N = (lower part >> 1) & 0x1F.
                512 << ((0x20 >> 1) & 0x1F) = 0x02000000 = 32 MiB virtual region,
                which includes mirrors since the NDS only has 32 KiB of physical ITCM.
                For DTCM, 512 << ((0x0a >> 1) & 0x1F) = 0x4000 = 16 KiB.
                
                These also don't actually enable either region, enabling the region is
                done by setting SCTLR:
                SCTLR bit 16: DTCM enable
                SCTLR bit 18: ITCM enable */
            options(nostack),
        );

        asm!(
            "mcr p15, 0, {icache}, c2, c0, 1", /* Instruction Cacheable (c2, c0, 1) -> 0x42 */
            "mcr p15, 0, {dcache}, c2, c0, 0", /* Data Cacheable (c2, c0, 0) -> 0x42 */
            "mcr p15, 0, {buf},    c3, c0, 0", /* Data Bufferable (c3, c0, 0) -> 0x02 */
            icache = in(reg) I_CACHE_REGIONS,
            dcache = in(reg) D_CACHE_REGIONS,
            buf    = in(reg) BUFFERABLE_REGIONS,
            options(nomem, nostack),
        );

        asm!(
            "mcr p15, 0, {iap}, c5, c0, 3",
            "mcr p15, 0, {dap}, c5, c0, 2",
            iap = in(reg) IAP_CONFIG,
            dap = in(reg) DAP_CONFIG,
            options(nomem, nostack),
        );

        let mut sctlr: u32;

        /* System Control Register (SCTLR), system register identified by
            CRn (Primary Register) = c1 is the group designated for System Control Registers,
            CRm (Secondary Register) = c0,
            opcode2 = 0 */
        asm!("mrc p15, 0, {0}, c1, c0, 0", out(reg) sctlr, options(nomem, nostack)); /* nomem = these operations do not access general memory */

        /* Set bits 0x0005707d (bits 0, 2, 3, 4, 5, 6, 12, 13, 14, 16, 18)
            bit 0   M   Protection unit (MMU) enable
            bit 1   A   Alignment fault check (1 will fault on unaligned access) (Since in ARMv5 ARM946E-S, unaligned 32-bit data reads don't abort, but instead load the aligned word and circularly rotate the byte into place, there is no abort generator for alignment at all, meaning this SBZ here)
            bit 2   C   DCache (data-cache) enable
            bit 3   W   Write buffer enable (On ARM946E-S this is set per-region so this global one SBO, according the manual)
            bit 4   P   32-bit Program space enable (reserved SBO, should be 1; for the transition from 26-bit addressing (ARMv2, ARMv3) to 32-bit addressing that was done by the time ARM946E-S became a thing)
            bit 5   D   32-bit Data space enable (reserved SBO, should be 1; for the transition from 26-bit addressing (ARMv2, ARMv3) to 32-bit addressing that was done by the time ARM946E-S became a thing)
            bit 6   L   Late Abort model select (reserved SBO, should be 1; for the transition from 26-bit addressing (ARMv2, ARMv3) to 32-bit addressing that was done by the time ARM946E-S became a thing)
            bit 7   B   Big-endian
            bit 8   S   System protection bit (SBZ)
            bit 9   R   ROM protection bit (SBZ)
            ...reserved SBZ
            bit 12  I   ICache (instruction-cache) enable
            bit 13  V   Alternate vector select (High Vectors, Exception vectors mapped to 0xffff0000)
            bit 14  RR  Round-robin replacement (Cache replacement strategy)
            bit 15  L4  Configure disable loading TBIT (with this enabled, data loads into the PC no longer checks the TBIT to switch to Thumb, but you can still switch modes with explicit bx and blx)
            bit 16  DT  Data RAM (DTCM) enable
            bit 17  DL  Data RAM load mode (enable. Since DTCM is layered *over* existing memory, a section of existing memory is basically inaccessible depending on whether anything was mapped there by the External Bus. Turning this on goes around DTCM and lets accesses to the DTCM address space *by the Data Bus (D-Side)* to fall through to the covered External Bus (Instruction Fetch targeting DTCM range goes through External Bus no matter what, the I-side does not have the physical wiring to access DTCM SRAM). Disabled when running real code since it defeats the point of fast DTCM)
            bit 18  IT  Instruction RAM (ITCM) enable
            bit 19  IL  Instruction RAM load mode (enable. Since ITCM is layered *over* existing memory, a section of existing memory is basically inaccessible depending on whether anything was mapped there by the External Bus. Turning this on goes around ITCM and lets accesses to the ITCM address space *by the Instruction Bus (I-Side)* to fall through to the covered External Bus (Data Access by the D-side like ldr and str *still gets routed to the ITCM SRAM*). Disabled when running real code since it defeats the point of fast ITCM)
            ...reserved SBZ */
        sctlr |= 0x0005707d;
        
        asm!("mcr p15, 0, {0}, c1, c0, 0", in(reg) sctlr, options(nomem, nostack)); /* Write back into SCTLR */
    }

    unsafe {
        asm!(
            "bx lr",
            options(noreturn, nostack),
        );
    }
}

unsafe extern "C" {
    static __svc_stack_top: u8;
    static __svc_irq_stack_boundary: u8;
    static __irq_stack_size: u8;
}

const MODE_SVC: u32 = 19; //0x13
const MODE_IRQ: u32 = 18; //0x12
const MODE_SYS: u32 = 31; //0x1F

/// Ensures SP ≡ 4 (mod 8), so that pushing an odd-number of words onto
/// the stack will land SP on an 8-byte boundary, satisfying AAPCS.
#[inline(always)]
pub fn align_sp_4_mod_8() {
    unsafe {
        core::arch::asm!(
            "tst sp, #4",
            "subeq sp, sp, #4",
            options(nostack),
        );
    }
}

/// Ensures SP ≡ 0 (mod 8), so that SP lands on an 8-byte boundary, satisfying AAPCS.
#[inline(always)]
pub fn align_sp_0_mod_8() {
    unsafe {
        core::arch::asm!(
            "tst sp, #4",
            "subne sp, sp, #4",
            options(nostack),
        );
    }
}

#[inline(always)]
fn init_banked_stacks() {
    unsafe {
        let mut svc_irq_stack_boundary: u32;

        asm!(
            // 1. Supervisor (SVC) Mode
            "msr CPSR_c, {svc_mode}",
            "ldr sp, ={svc_top}",

            // 2. IRQ Mode
            "msr CPSR_c, {irq_mode}",
            "ldr r0, ={svc_irq_stack_boundary}",
            "sub sp, r0, #4",

            svc_mode = const MODE_SVC,
            irq_mode = const MODE_IRQ,
            svc_top = sym __svc_stack_top,
            svc_irq_stack_boundary = sym __svc_irq_stack_boundary,
            out("r0") svc_irq_stack_boundary,
        );

        align_sp_4_mod_8();

        asm!(
            "ldr r1, ={irq_size}",
            "sub r1, r0, r1",

            // 3. System (SYS) Mode
            "msr CPSR_fsxc, {sys_mode}",
            "sub sp, r1, #4",

            sys_mode = const MODE_SYS,
            irq_size = sym __irq_stack_size,
            in("r0") svc_irq_stack_boundary,
            out("r1") _,
        );
    }
    // Note: No, I will not use naked asm or global asm. This works, nothing spills to stack.
}

#[inline(always)]
fn fill_volatile(value: u32, mut dst: *mut u32, mut size: usize) -> *mut u32 {
    debug_assert_eq!(size % core::mem::size_of::<u32>(), 0);
    while size != 0 {
        unsafe {
            core::arch::asm!(
                "str {value}, [{dst}], #4",
                dst = inout(reg) dst,
                value = in(reg) value,
                options(nostack, preserves_flags),
            );
        }
        size -= 4;
    }
    dst
}

unsafe extern "C" {
    static __dtcm_clear_size: u8;
    static __palette_ram_clear_size: u8;
    static __oam_fill_size: u8;
}

#[repr(C)]
struct BuildInfo {
    autoload_infos_start: *const u32,
    autoload_infos_end: *const u32,
    autoload_blocks: *const u32,
    bss_start: *const u32,
    bss_end: *const u32,
    compressed_code_ptr: *const u32,
    sdk_version: u32,
    nitrocode: u32,
    nitrocode_rev: u32,
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
fn uncompress_backward(ptr: *const u32) {
    if ptr.is_null() {
        return;
    }

    /* Useful diagram:
        r1 (stop)           r3 (read ←)        r0 (footer)       r2 (write ←)
        |                     |                   |                  |
        v                     v                   v                  v
        [===compressed data===|------footer-------|....expansion.....] */
    unsafe {
        let buffer_expansion_size: u32;
        let packed_header: u32;

        asm!(
            "ldmdb {ptr}, {{r1, r2}}",
            ptr = in(reg) ptr,
            out("r1") packed_header, /* r1 = [r0-8] (packed header) */
            out("r2") buffer_expansion_size, /* r2 = [r0-4] (buffer expansion size) */
        );

        let mut write_pointer: *mut u8;
        let mut read_pointer: *const u8;
        let stop_address: *const u8;
        let buffer_end_address_copy: *const u8;

        asm!(
            "add  r2, {ptr}, r2",           /* r2 = write pointer (top of decompressed buffer) */
            "sub  r3, {ptr}, r1, lsr #24",  /* r3 = read pointer (r1's top byte is an offset) */
            "bic  r1, r1, #0xFF000000",     /* clear said top byte from r1 */
            "sub  r1, {ptr}, r1",           /* r1 = stop address (bottom of buffer) */
            "mov  r4, r2",                  /* r4 = save copy of buffer end address for flushing the cache later */
            ptr = in(reg) ptr,
            inout("r1") packed_header => stop_address,
            inout("r2") buffer_expansion_size => write_pointer,
            out("r3") read_pointer,
            out("r4") buffer_end_address_copy,
        );

        'outer: while read_pointer > stop_address {
            let mut flag_byte: u32;
            
            asm!(
                "ldrb	r5, [r3, #-1]!",
                inout("r3") read_pointer,
                out("r5") flag_byte,
            );

            let mut i: i32 = 8; /* 8 bits in a byte */

            loop {
                i = i - 1;

                if i.is_negative() {
                    break;
                }

                if flag_byte & 0x80 != 0 { /* back-reference */
                    let mut displacement: u32;
                    let mut length: isize;

                    asm!(
                        "ldrb  ip, [r3, #-1]!",  /* high byte of the 16-bit reference token */
                        "ldrb  r7, [r3, #-1]!",  /* low byte */
                        /* the 16-bit token is packed as LLLL OOOO OOOOOOOO, where LLLL is 4 bits of length, and the rest are 12 bits of offset */
                        "orr   r7, r7, ip, lsl #8", /* r7 now contains the full 16-bit value */

                        "bic   r7, r7, #0xF000", /* clear LLLL to get r7 = bottom 12 bits = displacement */
                        "add   r7, r7, #2",      /* displacement += 2 (minimum distance of 2) */
                        "add   ip, ip, #32",
                        /* We add by 0x20 here and later subtract by 0x10 per iteration.
                            That never touches the bits & 0x0F (the lower 4 bits) until the
                            *upper* 4 bits underflow, at which point the whole value becomes
                            negative since the value loaded in the register was a single byte
                            and there aren't any more significant bits set, and that's when
                            it stops. So ignoring the lower 4 bits and treating the upper 4 bits
                            like the count works. */

                        inout("r3") read_pointer,
                        out("r7") displacement,
                        out("ip") length,
                    );

                    asm!(
                        "1:",
                        "ldrb  r0, [r2, r7]",   /* read the already-decompressed value at (write_ptr + displacement) */
                        "strb  r0, [r2, #-1]!", /* write */
                        "subs  {len}, {len}, #16", /* the subtraction by 0x10 mentioned above */
                        "bge 1b",
                        len = inlateout(reg) length => length,
                        in("r7") displacement,
                        inout("r2") write_pointer,
                        out("r0") _,
                        options(nostack),
                    );
                } else { /* literal */
                    asm!(
                        "ldrb  r0, [r3, #-1]!", /* read */
                        "strb  r0, [r2, #-1]!", /* write */
                        inout("r3") read_pointer,
                        inout("r2") write_pointer,
                        out("r0") _,
                    );
                }

                #[inline(always)]
                fn _lsl(flag_byte: &mut u32) {
                    unsafe {
                        asm!( /* repeated on purpose */
                            "lsl {f}, {f}, #1",
                            f = inout(reg) *flag_byte,
                        );
                    }
                }

                if read_pointer <= stop_address {
                    _lsl(&mut flag_byte);
                    break 'outer;
                } else {
                    _lsl(&mut flag_byte);
                }
            }
        }

        synchronize_executable_cache_lines_in_range(stop_address, buffer_end_address_copy);
    }
}

#[inline(always)]
fn synchronize_executable_cache_lines_in_range(start: *const u8, stop: *const u8) {
    unsafe {
        let mut start_address_cache_line_aligned: *const u8;

        asm!(
            "bic  {aligned}, {start_address}, #31", /* align down to 32-byte cache line */
            start_address = in(reg) start,
            aligned = out(reg) start_address_cache_line_aligned,
        );

        /* Since the data write synchronized is executable code, there are three
            systems to touch in order.
            First, the Clean + invalidate D-cache line. Since D-cache handles writeable
            memory, the cleaning steps takes dirty lines and pushes them into the write
            buffer.
            Second, once all those write commands are issued on the write buffer, waiting
            on Drain write buffer (CP15DSB) ensures that those writes have actually been
            written.
            Third, we can finally now Invalidate the I-cache line. With the instruction
            cache invalidated, the I-Side will fetch code from memory again, and since we have
            already waited on the Drain write buffer, we know that it will read the newly written
            data.

            So here, we will walk every 32-byte cache line and perform those steps.
            
            Now, you might notice here a difference in the implementation with my
            explanation. We appear to first do a Drain write buffer, and then invalidate the
            *I-cache* instead of the D-cache first, only cleaning + invalidating the D-cache
            after. Shouldn't this mean that for each cache-line, there are two problems?
            One that when we issue that Drain write buffer barrier, the write commands for this
            cache-line haven't been issued yet because we haven't started a D-cache clean.
            And two that we are invalidating the I-cache *before* the D-cache, telling the I-cache
            to fetch instructions again before we are sure that the newly written instructions
            are visible to it?
            And you'd be right, but for some reason, the original game orders it like this and nothing
            is visibly breaking, so we'll leave it like this. This only affects the very last cache-
            line we touch, and data written here isn't run a good amount of time later anyways so
            even that last cache-line has probably updated by the time we will be reading, which is
            probably why it works anyways. But it does look like someone thought "let's make sure
            that the code that just got decompressed gets seen," read through the ARM technical reference manual,
            and put all the instructions related to cache cleaning here and thought, "surely that covers it." */
        loop {
            asm!(
                /* CRn = c7 is the group designated for Cache, Branch Predictor, and Memory Barrier operations
                    CRm = c10 groups cache clean operations and pipeline/bus stall commands */
                "mcr p15, 0, {0}, c7, c10, 4",  // Drain write buffer (CP15DSB)
                /*  CRm = c5 targets I-Cache */
                "mcr p15, 0, {current}, c7, c5, 1",  // Invalidate I-cache line at current
                "mcr p15, 0, {current}, c7, c14, 1", // Clean + invalidate D-cache line at current
                in(reg) 0,
                current = in(reg) start_address_cache_line_aligned,
                options(nostack),
            );

            start_address_cache_line_aligned = start_address_cache_line_aligned.add(32); /* next cache line */
        
            if start_address_cache_line_aligned >= stop {
                break;
            }
        }
    }
}

unsafe extern "C" {
    static __build_info: u8;
}

#[inline(always)]
fn copy_range_volatile(
    mut src_start: *const u32,
    src_end: *const u32,
    mut dst: *mut u32,
) -> *mut u32 {
    while src_start < src_end {
        unsafe {
            core::arch::asm!(
                "ldr {scratch}, [{src}], #4",
                "str {scratch}, [{dst}], #4",
                src = inout(reg) src_start,
                dst = inout(reg) dst,
                scratch = out(reg) _,
                options(nostack, preserves_flags),
            );
        }
    }
    dst
}

#[inline(always)]
fn zero_fill_volatile(mut dst: *mut u32, mut size: usize) -> *mut u32 {
    debug_assert_eq!(size % core::mem::size_of::<u32>(), 0);
    while size != 0 {
        unsafe {
            core::arch::asm!(
                "str {zero}, [{dst}], #4",
                dst = inout(reg) dst,
                zero = in(reg) 0u32,
                options(nostack, preserves_flags),
            );
        }
        size -= 4;
    }
    dst
}

#[repr(C)]
struct AutoloadEntry {
    dest_addr: *mut u32,
    stored_size: u32,
    bss_size: u32,
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn do_autoload() -> ! {
    let build_info = unsafe { &*(linker_addr(&raw const __build_info) as *const BuildInfo) };
    let &BuildInfo { autoload_infos_start: mut current_entry_ptr, autoload_infos_end, autoload_blocks: mut current_src_start, .. } = build_info;
    unsafe { asm!("", inout("r1") current_entry_ptr); }
    unsafe { asm!("", in("r2") autoload_infos_end); }
    unsafe { asm!("", inout("r3") current_src_start); }

    while current_entry_ptr != autoload_infos_end {
        let autoload_entry = unsafe { &*(current_entry_ptr as *const AutoloadEntry) };
        let &AutoloadEntry { dest_addr: dest_addr_start, stored_size, bss_size } = autoload_entry;
        unsafe { asm!("", in("r0") dest_addr_start); }
        unsafe { asm!("", in("r4") stored_size); }
        unsafe { asm!("", in("r5") bss_size); }

        let current_src_end = unsafe { current_src_start.byte_add(stored_size as usize) };
        unsafe { asm!("", in("r8") current_src_end); }
        let mut dest_addr = copy_range_volatile(
            current_src_start,
            current_src_end,
            dest_addr_start,
        );
        unsafe { asm!("", in("r7") dest_addr); }

        current_src_start = current_src_end;

        unsafe {
            // bss
            dest_addr = zero_fill_volatile(
                dest_addr,
                bss_size as usize,
            );

            synchronize_executable_cache_lines_in_range(dest_addr_start as *const u8, dest_addr as *const u8);

            current_entry_ptr = current_entry_ptr.add(3);
        }
    }

    unsafe {
        asm!(
            "bx lr",
            options(noreturn, nostack),
        );
    }
}

#[inline(always)]
fn zero_fill_range_volatile(
    mut dst_start: *mut u32,
    dst_end: *mut u32,
) -> *mut u32 {
    debug_assert_eq!((dst_end as u32 - dst_start as u32) as usize % core::mem::size_of::<u32>(), 0);
    while dst_start < dst_end {
        unsafe {
            core::arch::asm!(
                "str {zero}, [{dst}], #4",
                dst = inout(reg) dst_start,
                zero = in(reg) 0u32,
                options(nostack, preserves_flags),
            );
        }
    }
    dst_start
}

#[inline(always)]
fn clear_module_bss() {
    let build_info = unsafe { &*(linker_addr(&raw const __build_info) as *const BuildInfo) };
    let &BuildInfo { bss_start, bss_end, .. } = build_info;
    unsafe { asm!("", in("r1") bss_start); }
    unsafe { asm!("", in("r2") bss_end); }
    let bss_start_copy = bss_start;
    // bss
    let fill_end = zero_fill_range_volatile(bss_start as *mut u32, bss_end as *mut u32);
    // sync
    synchronize_executable_cache_lines_in_range(bss_start_copy as *const u8, fill_end as *const u8);
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn _start() -> ! {
    let mut reg_base = linker_addr(&raw const __reg_base);
    unsafe { asm!("", inout("ip") reg_base); /* prevent llvm constant-folding */ }
    ime_off(reg_base);
    wait_scanline_0(reg_base);
    unsafe {
        asm!(
            "bl {}",
            sym init_cp15,
            clobber_abi("C"),
            options(nostack),
        );
    }
    init_banked_stacks();
    fill_volatile(
        0u32, /* btw, since stacks are all inside dtcm this clears that stack, though since nothing pushes onto the stack yet that is fine (same as original) (but ensure crt0 doesn't accidentally push or use the stack) */
        linker_addr(&raw const __dtcm_base) as *mut u32,
        linker_addr(&raw const __dtcm_clear_size) as usize,
    );
    fill_volatile(
        0u32,
        linker_addr(&raw const __palette_ram_base) as *mut u32,
        linker_addr(&raw const __palette_ram_clear_size) as usize,
    );
    fill_volatile(
        0x200,
        linker_addr(&raw const __oam_base) as *mut u32,
        linker_addr(&raw const __oam_fill_size) as usize,
    );
    let build_info = unsafe { &*(linker_addr(&raw const __build_info) as *const BuildInfo) };
    let &BuildInfo { compressed_code_ptr, .. } = build_info;
    uncompress_backward(compressed_code_ptr);
    unsafe {
        asm!(
            "bl {}",
            sym do_autoload,
            /* do_autoload uses some callee-saved non-volatile registers (r4-r11),
                which need to be preserved usually by the called function
                if it wants to use them, but since here we are using a
                manual `bx lr` to return, LLVM does not know that, so
                we need to tell LLVM which registers will be clobbered. */
            out("r4") _,
            out("r5") _,
            out("r7") _,
            out("r8") _,
            clobber_abi("C"),
            options(nostack),
        );
    }
    clear_module_bss();
    panic!();
}

