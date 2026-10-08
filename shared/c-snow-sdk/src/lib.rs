#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]

use core::cell::UnsafeCell;

use utils::prelude::*;

pub mod asm;

pub mod consts;

pub mod reg;

pub mod dtcm;

pub mod os;

pub mod pxi;

pub mod sys;

pub mod svc;

#[cfg(armv4t)]
#[unsafe(link_section = ".text.wram")]
#[unsafe(no_mangle)]
pub extern "C" fn HardwareInterrupt() -> ! {
    core::hint::black_box(2+2);
    panic!();
}

#[cfg(armv4t)]
#[unsafe(link_section = ".text.wram")]
#[unsafe(no_mangle)]
pub extern "C" fn SnowSpMain() -> ! {
    core::hint::black_box(1+1);
    panic!();
}

#[cfg(armv5te)]
#[unsafe(link_section = ".text.itcm")]
#[unsafe(no_mangle)]
pub extern "C" fn HardwareInterrupt() -> ! {
    core::hint::black_box(2+2);
    panic!();
}

#[cfg(armv5te)]
#[unsafe(link_section = ".text.origaddr.02000C6C")]
#[unsafe(no_mangle)]
pub extern "C" fn SnowMain() -> ! {
    core::hint::black_box(1+1);
    panic!();
}

#[inline(always)]
pub fn linker_addr(symbol: *const u8) -> u32 {
    symbol as u32
}

/// Wrapper type for a global variable.
///
/// # Safety
///
/// If the global is accessed by both CPUs, synchronization needs
/// to be handled carefully.
///
/// Also, **even if a value is only accessed by one CPU, there could
/// *still be sync issues!!!*** Namely, IRQ (interrupts) can still preempt
/// normal code, which means that memory might change in between say a
/// read-compare-write sequence of instructions, where the IRQ might
/// happen to preempt after the read. In those cases, the pattern is
/// to disable IRQ first, do the sequence, and then to re-enable
/// it afterwards.
///
/// If the global is touched by neither an interrupt handler nor the
/// other CPU, it can be treated like a local.
///
/// ## I-Cache and D-Cache
///
/// While both [`get`](Global::get) and [`set`](Global::set) on [`Global`]
/// is volatile, this is a separate idea from and does **not** prevent ARM9's
/// caching unit from providing stale data for shared values with ARM7 or DMA
/// if it is changed under you.
///
/// Though this is less of a concern since in practice, cacheability and bufferability settings in _start
/// are configured such that only specific regions of virtual memory addresses get cached and buffered.
///
/// Effectively,
///
/// **Data Side:**
///
/// | Address range | Winning region | Cached | Write-back | Access |
/// |---|---|---|---|---|
/// | 0x02000000–0x027DFFFF | 1         | yes | yes | RW |
/// | 0x027E0000–0x027E3FFF | 4 (DTCM)  | no* | no | RW |
/// | 0x027E4000–0x027FEFFF | 2         | no | no | no access |
/// | 0x027FF000–0x027FFFFF | 7         | no | no | RW |
///
/// *In addition:*
/// - Region 0 (I/O, palette, VRAM, OAM), region 3 (GBA slot), and region 5 (ITCM mirrors), are all
/// uncached, with privileged read/write.
/// - Region 6 (BIOS) is cached, but no write-back as it is privileged read-only (and etched to
/// silicon as well).  (B=0, technically write-through, but there's no way to write)
/// - And **everything not covered by a region, including shared WRAM at 0x03000000–0x03FFFFFF,
/// has no access at all, surprisingly.** So here, shared WRAM can not be accessed by ARM9.
/// This is mentioned in GBATEK when it discusses its default table, "Above settings do not allow to access Shared Memory at 37F8000h?".
///
/// \* TCM accesses literally never go through the cache physically, but GBATEK
/// recommends turning cache and the write buffer off for TCM regions anyways, which
/// is what regions 4 and 5 do here.
///
/// From the ARM946E-S Technical Reference Manual:
///
/// > When you have enabled the I-SRAM, all future ARM9E-S
/// instruction fetches and data accesses to the I-SRAM address
/// space cause the I-SRAM to be accessed.
///
/// > *When you have enabled the D-SRAM, see Register 9,
/// Tightly-coupled memory region registers on page* 2-25,
/// all future read and write accesses to the D-SRAM address
/// space cause the D-SRAM to be accessed.
///
/// **Instruction Side:**
///
/// | Address range | Winning region | I-cache | Fetch allowed | Notes |
/// |---|---|---|---|---|
/// | 0x00000000–0x00FFFFFF | none (background) | – | no | Lower ITCM mirrors acting as a null-pointer guard (a jump to 0 faults) |
/// | 0x01000000–0x01FFFFFF | 5 | no | privileged | ITCM mirrors, with code here starting at 0x01FF8000 |
/// | 0x02000000–0x027DFFFF | 1 | yes | privileged | Both mirrors of Main RAM, including 0x023E0000–0x023FFFFF, the lower mirror of the ARM7-private and system areas |
/// | 0x027E0000–0x027E3FFF | 4 | no | no | The fetch would bypass the DTCM and hit ARM7-private main RAM since the Instruction Bus (I-Side) is not connected to the DTCM |
/// | 0x027E4000–0x027FEFFF | 2 | no | no | More ARM7-private main RAM |
/// | 0x027FF000–0x027FFFFF | 7 | no | no | System and SDK area |
/// | 0x02800000–0x03FFFFFF | none | – | no | Including the shared WRAM at 0x03000000+ |
/// | 0x04000000–0x07FFFFFF | 0 | no | privileged | I/O, palette, VRAM, OAM; executing from VRAM is permitted |
/// | 0x08000000–0x0FFFFFFF | 3 | no | no | GBA slot |
/// | 0x10000000–0xFFFEFFFF | none | – | no | |
/// | 0xFFFF0000–0xFFFF7FFF | 6 | yes | privileged (read-only) | BIOS, exception vectors |
/// | 0xFFFF8000–0xFFFFFFFF | none | – | no | |
///
/// **However,** with that said, it is still entirely possible that
/// code might want to use a buffer or a piece of data within the cached or buffered
/// memory ranges and still want to have it be accessed from another CPU. Be
/// aware of when any memory location is being shared, and do a quick check.
#[repr(transparent)]
pub struct Global<T>(UnsafeCell<T>);

unsafe impl<T> Sync for Global<T> {}

impl<T> Global<T> {
    pub const fn new(value: T) -> Self {
        Self(UnsafeCell::new(value))
    }

    pub const fn as_ptr(&self) -> *mut T {
        self.0.get()
    }

    /// # Safety
    ///
    /// **ASSUMES LOCAL.**
    ///
    /// Do not nest on the same value or access the same variable again using
    /// any other method while the reference provided is alive (such as with direct [`get`](Global::get) or [`set`](Global::set)).
    /// This would would create two mutable references to the same memory, or
    /// otherwise interact with the value from a second reference when Rustlang
    /// only expects the one `&mut T`, causing undefined behavior in Rust.
    pub unsafe fn with<R>(&self, f: impl FnOnce(&mut T) -> R) -> R {
        f(unsafe { &mut *self.0.get() })
    }
}

impl<T: Copy> Global<T> {
    #[inline(always)]
    pub fn get(&self) -> T {
        unsafe { core::intrinsics::volatile_load(self.0.get()) }
    }

    #[inline(always)]
    pub fn set(&self, value: T) -> T {
        unsafe { core::intrinsics::volatile_store(self.0.get(), value) }
        value
    }
}

/// Raw read
#[inline(always)]
pub fn read<T>(src: *const T) -> T {
    unsafe { core::intrinsics::volatile_load(src) }
}

/// Raw write
#[inline(always)]
pub fn write<T: Copy>(dst: *mut T, val: T) -> T {
    unsafe { core::intrinsics::volatile_store(dst, val) }
    val
}

#[inline(always)]
pub const fn cp15_size_encoding(size: u32) -> u32 {
    assert!(size >= 0x1000);
    assert!(size.is_power_of_two());

    (size.trailing_zeros() - 1) << 1  /* PMSAv5 size encoding */
}

#[macro_export]
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

