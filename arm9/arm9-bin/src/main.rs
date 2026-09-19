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
fn linker_addr(symbol: *const u8) -> usize {
    symbol as usize
}

const REG_BASE: u32 = 0x04000000;

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
            bit 0   M   MMU enable
            bit 2   C   data-cache enable
            bit 12  I   instruction-cache enable */
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

        let dtcm_base = linker_addr(&raw const __dtcm_base) as u32;
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
    }

    unsafe {
        asm!(
            "bx lr",
            options(noreturn, nostack),
        );
    }
}

#[unsafe(no_mangle)]
#[unsafe(link_section = ".text.crt0")]
pub extern "C" fn _start() -> ! {
    let mut reg_base = REG_BASE;
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
    panic!();
}

