#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![allow(internal_features)]
#![allow(non_upper_case_globals)]
#![allow(non_snake_case)]

use core::{cell::UnsafeCell, marker::PhantomData, ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive}};

use utils::prelude::*;

pub mod consts;

pub mod os;

pub mod sys;

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

pub trait RWT {
    unsafe fn read<T>(src: *const T) -> T;
    unsafe fn write<T: Copy>(dst: *mut T, val: T) -> T;
}

#[derive(Clone, Copy)]
pub struct VolatileRW;

#[derive(Clone, Copy)]
pub struct UnalignedVolatileRW;

#[derive(Clone, Copy)]
pub struct UnalignedRW;

#[derive(Clone, Copy)]
pub struct RegularRW;

impl RWT for VolatileRW {
    #[inline(always)]
    unsafe fn read<T>(src: *const T) -> T {
        unsafe { core::intrinsics::volatile_load(src) }
    }

    #[inline(always)]
    unsafe fn write<T: Copy>(dst: *mut T, val: T) -> T {
        unsafe { core::intrinsics::volatile_store(dst, val) }
        val
    }
}

impl RWT for UnalignedVolatileRW {
    #[inline(always)]
    unsafe fn read<T>(src: *const T) -> T {
        unsafe { core::intrinsics::unaligned_volatile_load(src) }
    }

    #[inline(always)]
    unsafe fn write<T: Copy>(dst: *mut T, val: T) -> T {
        unsafe { core::intrinsics::unaligned_volatile_store(dst, val) }
        val
    }
}

#[repr(C, packed)]
struct Unaligned<T>(T);

impl RWT for UnalignedRW {
    #[inline(always)]
    unsafe fn read<T>(src: *const T) -> T {
        unsafe { core::intrinsics::read_via_copy(src as *const Unaligned<T>).0 }
    }

    #[inline(always)]
    unsafe fn write<T: Copy>(dst: *mut T, val: T) -> T {
        unsafe { core::intrinsics::write_via_move(dst as *mut Unaligned<T>, Unaligned(val)) }
        val
    }
}

impl RWT for RegularRW {
    #[inline(always)]
    unsafe fn read<T>(src: *const T) -> T {
        unsafe { core::intrinsics::read_via_copy(src) }
    }

    #[inline(always)]
    unsafe fn write<T: Copy>(dst: *mut T, val: T) -> T {
        unsafe { core::intrinsics::write_via_move(dst, val) }
        val
    }
}

/// [`Volatile`] is volatile-by-default since that's 99% of the use-cases;
/// we follow an "only opt out of volatile where it's needed" approach here.
/// But for when the other read-write modes are justified, there are [`From`] impls to
/// change the nature of the wrapper.
#[repr(transparent)]
pub struct Volatile<T, RW: RWT = VolatileRW> {
    ptr: *const T,
    _rw: PhantomData<RW>,
}

impl<T: Copy, RW: RWT> Volatile<T, RW> {
    #[inline(always)]
    pub const fn new(ptr: *const T) -> Self {
        Self { ptr, _rw: PhantomData }
    }

    #[inline(always)]
    pub const fn with_rw<NewRW: RWT>(self) -> Volatile<T, NewRW> {
        Volatile { ptr: self.ptr, _rw: PhantomData }
    }

    #[inline(always)]
    pub fn get(&self) -> T {
        unsafe { RW::read(self.ptr) }
    }
}

/// [`VolatileMut`] is volatile-by-default since that's 99% of the use-cases;
/// we follow an "only opt out of volatile where it's needed" approach here.
/// But for when the other read-write modes are justified, there are [`From`] impls to
/// change the nature of the wrapper.
#[repr(transparent)]
pub struct VolatileMut<T, RW: RWT = VolatileRW> {
    ptr: *mut T,
    _rw: PhantomData<RW>,
}

impl<T: Copy, RW: RWT> VolatileMut<T, RW> {
    #[inline(always)]
    pub const fn new(ptr: *mut T) -> Self {
        Self { ptr, _rw: PhantomData }
    }

    #[inline(always)]
    pub const fn with_rw<NewRW: RWT>(self) -> VolatileMut<T, NewRW> {
        VolatileMut { ptr: self.ptr, _rw: PhantomData }
    }

    #[inline(always)]
    pub fn get(&self) -> T {
        unsafe { RW::read(self.ptr) }
    }

    #[inline(always)]
    pub fn set(&self, value: T) -> T {
        unsafe { RW::write(self.ptr, value) }
    }
}

macro_rules! impl_from_volatile_default {
    ($wrapper:ident => $($target:ty),+ $(,)?) => {
        $(
            impl<T> From<$wrapper<T, VolatileRW>> for $wrapper<T, $target> {
                #[inline(always)]
                fn from(v: $wrapper<T, VolatileRW>) -> Self {
                    Self { ptr: v.ptr, _rw: PhantomData }
                }
            }
        )+
    };
}

impl_from_volatile_default!(Volatile => UnalignedVolatileRW, UnalignedRW, RegularRW);
impl_from_volatile_default!(VolatileMut => UnalignedVolatileRW, UnalignedRW, RegularRW);

impl<T, RW: RWT> From<VolatileMut<T, RW>> for Volatile<T, RW> {
    #[inline(always)]
    fn from(v: VolatileMut<T, RW>) -> Self {
        Self { ptr: v.ptr, _rw: PhantomData }
    }
}

unsafe impl<T: Send, RW: RWT> Send for Volatile<T, RW> {}
unsafe impl<T: Send, RW: RWT> Sync for Volatile<T, RW> {}

unsafe impl<T: Send, RW: RWT> Send for VolatileMut<T, RW> {}
unsafe impl<T: Send, RW: RWT> Sync for VolatileMut<T, RW> {}

impl<T, RW: RWT> Clone for Volatile<T, RW> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self { ptr: self.ptr, _rw: PhantomData }
    }
}
impl<T, RW: RWT> Copy for Volatile<T, RW> {}

impl<T, RW: RWT> Clone for VolatileMut<T, RW> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self { ptr: self.ptr, _rw: PhantomData }
    }
}
impl<T, RW: RWT> Copy for VolatileMut<T, RW> {}

pub trait IntoVolatile {
    type IntoVolatile;
    fn into_volatile(self) -> Self::IntoVolatile;
}

impl<T: Copy> IntoVolatile for *const T {
    type IntoVolatile = Volatile<T>;
    #[inline(always)]
    fn into_volatile(self) -> Self::IntoVolatile {
        Volatile::new(self)
    }
}

impl<T: Copy> IntoVolatile for *mut T {
    type IntoVolatile = VolatileMut<T>;
    #[inline(always)]
    fn into_volatile(self) -> Self::IntoVolatile {
        VolatileMut::new(self)
    }
}

/// Any raw pointer `*const T` or `*mut T`.
///
/// Since `wrapping_add` on both are implemented as inherent methods,
/// this trait gives access to those as universal trait methods.
pub trait Ptr: Copy {
    fn wrapping_add_(self, count: usize) -> Self;
}

impl<T> Ptr for *const T {
    #[inline(always)]
    fn wrapping_add_(self, count: usize) -> Self {
        self.wrapping_add(count)
    }
}

impl<T> Ptr for *mut T {
    #[inline(always)]
    fn wrapping_add_(self, count: usize) -> Self {
        self.wrapping_add(count)
    }
}

#[derive(Clone, Copy)]
pub struct KnownLen(pub usize);

#[derive(Clone, Copy)]
pub struct UnknownLen;

pub trait Len: Copy {
    fn check_index(self, idx: usize);
    fn check_range(self, start: usize, end: usize);
}

impl Len for KnownLen {
    #[inline(always)]
    #[track_caller]
    fn check_index(self, idx: usize) {
        debug_assert!(
            idx < self.0,
            "index out of bounds: the len is {} but the index is {}",
            self.0, idx,
        );
    }
    #[inline(always)]
    #[track_caller]
    fn check_range(self, start: usize, end: usize) {
        debug_assert!(
            start <= end,
            "slice index starts at {} but ends at {}",
            start, end,
        );
        debug_assert!(
            end <= self.0,
            "range end index {} out of range for array of length {}",
            end, self.0,
        );
    }
}

impl Len for UnknownLen {
    #[inline(always)]
    fn check_index(self, _: usize) {}
    #[inline(always)]
    fn check_range(self, _: usize, _: usize) {}
}

/// Any raw array or slice pointer.
///
/// Each form has different semantics, but there are useful ways to define,
/// "what is the address of the first element" and "what is the number of elements inside"
/// for each, so this provides a shared naming convention to access those.
///
/// This relies on inherent implementations taking precedence over trait ones.
///
/// The behavior is different for each form the array pointer takes:
///
/// #### `*const [T; N]`
/// - First element addr: `self.0.cast()` -> `*const T`
/// - Num elements: `KnownLen(N)` with `N` taken from the type.
///
/// #### `*mut [T; N]`
/// - First element addr: `self.0.cast()` -> `*mut T`
/// - Num elements: `KnownLen(N)` with `N` taken from the type.
///
/// #### `*const [T]`
/// - First element addr: `self.0.cast()` -> `*const T`
/// - Num elements: `KnownLen(self.0.len())` (fat-pointer)
///
/// #### `*mut [T]`
/// - First element addr: `self.0.cast()` -> `*mut T`
/// - Num elements: `KnownLen(self.0.len())` (fat-pointer)
///
/// #### `*const T` (array pointer with unknown length)
/// - First element addr: `self.0` -> `*const T`
/// - Num elements: **`UnknownLen`**
///
/// #### `*mut T` (array pointer with unknown length)
/// - First element addr: `self.0` -> `*mut T`
/// - Num elements: **`UnknownLen`**
#[derive(Clone, Copy)]
pub struct Wrap<P>(pub P);

pub trait DefaultWrap<T>: Copy
where
    T: Sized  /* explicitly writing it here even though it's the default because it's *very important* that whatever type it is, that this is Sized */
{
    type ElemPtr: Ptr;
    fn split(self) -> (Self::ElemPtr, UnknownLen);
}

impl<T> DefaultWrap<T> for Wrap<*const T> {
    type ElemPtr = *const T;
    #[inline(always)]
    fn split(self) -> (Self::ElemPtr, UnknownLen) {
        (self.0, UnknownLen)
    }
}

impl<T> DefaultWrap<T> for Wrap<*mut T> {
    type ElemPtr = *mut T;
    #[inline(always)]
    fn split(self) -> (Self::ElemPtr, UnknownLen) {
        (self.0, UnknownLen)
    }
}

impl<T, const N: usize> Wrap<*const [T; N]> {
    #[inline(always)]
    pub fn split(self) -> (*const T, KnownLen) {
        (self.0.cast(), KnownLen(N))
    }
}

impl<T, const N: usize> Wrap<*mut [T; N]> {
    #[inline(always)]
    pub fn split(self) -> (*mut T, KnownLen) {
        (self.0.cast(), KnownLen(N))
    }
}

impl<T> Wrap<*const [T]> {
    #[inline(always)]
    pub fn split(self) -> (*const T, KnownLen) {
        (self.0.cast(), KnownLen(core::intrinsics::ptr_metadata(self.0)))
    }
}

impl<T> Wrap<*mut [T]> {
    #[inline(always)]
    pub fn split(self) -> (*mut T, KnownLen) {
        (self.0.cast(), KnownLen(core::intrinsics::ptr_metadata(self.0 as *const [T])))
    }
}

/// [`Raw::wrap`] does nothing and returns the value passed to it
#[derive(Clone, Copy)]
pub struct Raw;

/// [`Vol::wrap`] wraps any type `E` with an [`IntoVolatile`] impl
/// in a volatile wrapper of type `E::IntoVolatile`
#[derive(Clone, Copy)]
pub struct Vol;

pub trait VolWrap<E> {
    type Output;

    fn wrap(e: E) -> Self::Output;
}

impl<E> VolWrap<E> for Raw {
    type Output = E;

    #[inline(always)]
    fn wrap(e: E) -> E {
        e
    }
}

impl<E: IntoVolatile> VolWrap<E> for Vol {
    type Output = E::IntoVolatile;

    #[inline(always)]
    fn wrap(e: E) -> E::IntoVolatile {
        e.into_volatile()
    }
}

/// Yields `start + i * size_of::<T>()` for every `i` in the range.
#[derive(Clone)]
pub struct ElemPtrs<E, W = Raw> {
    base: E,
    range: Range<usize>,
    _w: PhantomData<W>,
}

impl<E> ElemPtrs<E, Raw> {
    #[inline(always)]
    fn new(base: E, range: Range<usize>) -> Self {
        Self { base, range, _w: PhantomData }
    }
}

impl<E: Ptr, W: VolWrap<E>> ElemPtrs<E, W> {
    #[inline(always)]
    fn remaining(&self) -> usize {
        if self.range.start < self.range.end {
            self.range.end.wrapping_sub(self.range.start)
        } else {
            0
        }
    }
}

impl<E: Ptr, W: VolWrap<E>> Iterator for ElemPtrs<E, W> {
    type Item = W::Output;

    #[inline(always)]
    fn next(&mut self) -> Option<W::Output> {
        let i = self.range.start;
        if i < self.range.end {
            self.range.start = i.wrapping_add(1);
            Some(W::wrap(self.base.wrapping_add_(i)))
        } else {
            None
        }
    }

    #[inline(always)]
    fn nth(&mut self, n: usize) -> Option<W::Output> {
        if n < self.remaining() {
            let i = self.range.start.wrapping_add(n);
            self.range.start = i.wrapping_add(1);
            Some(W::wrap(self.base.wrapping_add_(i)))
        } else {
            self.range.start = self.range.end;
            None
        }
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let n = self.remaining();
        (n, Some(n))
    }

    #[inline(always)]
    fn count(self) -> usize {
        self.remaining()
    }

    #[inline(always)]
    fn last(mut self) -> Option<W::Output> {
        self.next_back()
    }
}

impl<E: Ptr, W: VolWrap<E>> DoubleEndedIterator for ElemPtrs<E, W> {
    #[inline(always)]
    fn next_back(&mut self) -> Option<W::Output> {
        if self.range.start < self.range.end {
            let i = self.range.end.wrapping_sub(1);
            self.range.end = i;
            Some(W::wrap(self.base.wrapping_add_(i)))
        } else {
            None
        }
    }

    #[inline(always)]
    fn nth_back(&mut self, n: usize) -> Option<W::Output> {
        if n < self.remaining() {
            let i = self.range.end.wrapping_sub(1).wrapping_sub(n);
            self.range.end = i;
            Some(W::wrap(self.base.wrapping_add_(i)))
        } else {
            self.range.end = self.range.start;
            None
        }
    }
}

impl<E: Ptr, W: VolWrap<E>> ExactSizeIterator for ElemPtrs<E, W> {
    #[inline(always)]
    fn len(&self) -> usize {
        self.remaining()
    }
}

impl<E: Ptr, W: VolWrap<E>> core::iter::FusedIterator for ElemPtrs<E, W> {}

impl<E: Ptr> IntoVolatile for ElemPtrs<E, Raw>
where
    Vol: VolWrap<E>,
{
    type IntoVolatile = ElemPtrs<E, Vol>;
    #[inline(always)]
    fn into_volatile(self) -> ElemPtrs<E, Vol> {
        ElemPtrs { base: self.base, range: self.range, _w: PhantomData }
    }
}

#[diagnostic::on_unimplemented(
    message = "array with length `{Self}` cannot be indexed by `{I}`",
    label = "trying to index into an array of unknown size without an end index",
    note = "try indexing with one of `a..b`, `a..=b`, `..b`, `..=b`, or if you",
    note = "know the length of the field, change the type of the field",
    note = "to a fixed size array type [T; N] or slice [T] instead?"
)]
pub trait PtrIndex<I, E: Ptr>: Len {
    type Output;
    fn project(self, idx: I, first: E) -> Self::Output;
}

impl<E: Ptr, L: Len> PtrIndex<usize, E> for L {
    type Output = E;
    #[inline(always)]
    #[track_caller]
    fn project(self, idx: usize, first: E) -> E {
        self.check_index(idx);
        first.wrapping_add_(idx)
    }
}

impl<E: Ptr, L: Len> PtrIndex<Range<usize>, E> for L {
    type Output = ElemPtrs<E>;
    #[inline(always)]
    #[track_caller]
    fn project(self, r: Range<usize>, first: E) -> ElemPtrs<E> {
        self.check_range(r.start, r.end);
        ElemPtrs::new(first, r)
    }
}

impl<E: Ptr, L: Len> PtrIndex<RangeInclusive<usize>, E> for L {
    type Output = ElemPtrs<E>;
    #[inline(always)]
    #[track_caller]
    fn project(self, r: RangeInclusive<usize>, first: E) -> ElemPtrs<E> {
        let (start, end) = r.into_inner();
        debug_assert!(end != usize::MAX, "inclusive range end overflows");
        let end = end.wrapping_add(1);
        self.project(start..end, first)
    }
}

impl<E: Ptr, L: Len> PtrIndex<RangeTo<usize>, E> for L {
    type Output = ElemPtrs<E>;
    #[inline(always)]
    #[track_caller]
    fn project(self, r: RangeTo<usize>, first: E) -> ElemPtrs<E> {
        self.project(0..r.end, first)
    }
}

impl<E: Ptr, L: Len> PtrIndex<RangeToInclusive<usize>, E> for L {
    type Output = ElemPtrs<E>;
    #[inline(always)]
    #[track_caller]
    fn project(self, r: RangeToInclusive<usize>, first: E) -> ElemPtrs<E> {
        debug_assert!(r.end != usize::MAX, "inclusive range end overflows");
        self.project(0..r.end.wrapping_add(1), first)
    }
}

impl<E: Ptr> PtrIndex<RangeFrom<usize>, E> for KnownLen {
    type Output = ElemPtrs<E>;
    #[inline(always)]
    #[track_caller]
    fn project(self, r: RangeFrom<usize>, first: E) -> ElemPtrs<E> {
        self.project(r.start..self.0, first)
    }
}

impl<E: Ptr> PtrIndex<RangeFull, E> for KnownLen {
    type Output = ElemPtrs<E>;
    #[inline(always)]
    fn project(self, _: RangeFull, first: E) -> ElemPtrs<E> {
        ElemPtrs::new(first, 0..self.0)
    }
}

#[diagnostic::on_unimplemented(
    message = "automatic `ldr` from multiple pointers is judged hazardous and thus not implemented",
    note = "trying to automatically load from a whole range of pointers here (`{Self}`)"
)]
pub trait LdrIndex {}
impl LdrIndex for usize {}

#[doc(hidden)]
#[inline(always)]
pub fn __ldr_index<T: LdrIndex>(_: &T) {}

#[diagnostic::on_unimplemented(
    message = "indexing with range is not allowed in the middle of an access chain",
    note = "ranges are only allowed on the last access in a chain"
)]
pub trait SingleIndex {}
impl SingleIndex for usize {}

#[doc(hidden)]
#[inline(always)]
pub fn __single_index<T: SingleIndex>(_: &T) {}

/// Returns a raw pointer to a named struct field, or for
/// the last field in a hierarchical sequence of accesses that you want to read or write to,
/// "final field" or "final mut field" wraps the raw pointer
/// in a volatile read and read-write wrapper respectively
/// so you can call `.get` and `.set` on them immediately.
///
/// For array or slice fields, you can do "field[i]" to get the address of a single
/// element, or "field[range]" for an iterator over element pointers, or volatile wrappers
/// if "final" is used in either aforementioned case.
///
/// **Intuitively,** think of this as follows: Usually in Rustlang you have two ways to deal
/// with complex raw pointer types like structs that need a nested path. Either every single step
/// is safe, like "struct.field_1.subfield_1.subsubfield_1" where you need a safe wrapper at *every*
/// level, or you do what is essentially pointer arithmetic and lose a significant part of the
/// ergonomics of having defined the intermediate types in the first place. This is somewhere in-between.
/// The idea is that the pointer arithmetic part (the part where you go field to field until you get to
/// the field you wanna touch) is rarely problematic because you are not accessing anything yet, so that
/// part can be significantly improved ergonomically. Basic pointer-chasing too. Most data on bare
/// metal work like this, so no need to enforce that we have safe wrappers *every* level down.
#[macro_export]
macro_rules! raw_field {
    (@index $idx:expr, $array:expr) => {{
        #[allow(unused_imports)]
        use $crate::DefaultWrap as _;
        let (first, len_struct) = $crate::Wrap($array).split();
        $crate::PtrIndex::project(len_struct, $idx, first)
    }};

    ($base:expr, final mut $field:ident [$idx:expr]) => {
        $crate::IntoVolatile::into_volatile($crate::raw_field!($base, mut $field [$idx]))
    };
    ($base:expr, final $field:ident [$idx:expr]) => {
        $crate::IntoVolatile::into_volatile($crate::raw_field!($base, $field [$idx]))
    };
    ($base:expr, mut $field:ident [$idx:expr]) => {{
        #[allow(unused_unsafe)] unsafe { $crate::raw_field!(@index $idx, &raw mut (*($base)).$field) }
    }};
    ($base:expr, $field:ident [$idx:expr]) => {{
        #[allow(unused_unsafe)] unsafe { $crate::raw_field!(@index $idx, &raw const (*($base)).$field) }
    }};

    ($base:expr, final ldr $field:ident [$idx:expr]) => {
        $crate::IntoVolatile::into_volatile($crate::raw_field!($base, ldr $field [$idx]))
    };
    ($base:expr, ldr $field:ident [$idx:expr]) => {{
        if false { $crate::__ldr_index(&$idx); }
        $crate::read($crate::raw_field!($base, $field [$idx]))
    }};

    ($base:expr, final ldr $field:ident) => {
        $crate::IntoVolatile::into_volatile($crate::raw_field!($base, ldr $field))
    };
    ($base:expr, ldr $field:ident) => {
        $crate::read($crate::raw_field!($base, $field))
    };

    ($base:expr, final mut $field:ident) => {
        $crate::VolatileMut::new(#[allow(unused_unsafe)] unsafe { &raw mut (*($base)).$field })
    };
    ($base:expr, final $field:ident) => {
        $crate::Volatile::new(#[allow(unused_unsafe)] unsafe { &raw const (*($base)).$field })
    };
    ($base:expr, mut $field:ident) => {{
        #[allow(unused_unsafe)] unsafe { &raw mut (*($base)).$field }
    }};
    ($base:expr, $field:ident) => {{
        #[allow(unused_unsafe)] unsafe { &raw const (*($base)).$field }
    }};

    // Chaining arms: peel off the first access (which must not be `final` or a range)
    // and then feed its pointer back in as the base of the rest.
    ($base:expr, mut $field:ident [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!({
            if false { $crate::__single_index(&$idx); }
            $crate::raw_field!($base, mut $field [$idx])
        }, $($rest)+)
    };
    ($base:expr, ldr $field:ident [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!($base, ldr $field [$idx]), $($rest)+)
    };
    ($base:expr, $field:ident [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!({
            if false { $crate::__single_index(&$idx); }
            $crate::raw_field!($base, $field [$idx])
        }, $($rest)+)
    };
    ($base:expr, mut $field:ident~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!($base, mut $field), $($rest)+)
    };
    ($base:expr, ldr $field:ident~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!($base, ldr $field), $($rest)+)
    };
    ($base:expr, $field:ident~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!($base, $field), $($rest)+)
    };
    ($base:expr, final mut $field:ident $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
    ($base:expr, final ldr $field:ident $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
    ($base:expr, final $field:ident $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
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

