// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 adakite1
//
// Licensed under either of Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
// or the MIT license (http://opensource.org) at your option.

#![cfg_attr(not(test), no_std)]
#![feature(core_intrinsics)]
#![allow(internal_features)]

use core::{marker::PhantomData, ops::{Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive}};

/// All RWT types assuming alignment are supposed to assume alignment and
/// codegen to simple read and write and that is the case for the volatile ones,
/// but for plain read and write, an RW to `&raw const place` was producing `ldrb`s
/// and `strb`s. This did not happen at no optimizations, so looking at the MIR
/// it's actually an interesting situation. The original MIR is:
///
/// ```
/// _2 = &raw const ((*_1).1: u32);
/// _0 = plain_read::<u32>(move _2);
/// ```
///
/// Inside `plain_read`, the LowerIntrinsics pass had already lowered `read_via_copy`
/// to `_0 = copy (*src)`, so after the Inline pass we get:
///
/// ```
/// _2 = &raw const ((*_1).1: u32);
/// _0 = copy (*_2);          // *_2 : pointee u32 => align 4
/// ```
///
/// This is still fine, but then ReferencePropagation sees that we are doing a raw borrow
/// of `(*_1).1: u32` and then immediately dereferencing it, so it pastes that in
/// as an optimization (`*&x` to `x`):
///
/// ```
/// _0 = copy ((*_1).1: u32);  // deref of *const S => packed struct align 1
/// ```
///
/// ReferencePropagation does this whenever there's a `_2` which has a corresponding `(*_1).1`
/// and `_2` is only assigned to once; when it is then used in derefs as `*_2`, the pass
/// replaces those uses with `(*_1).1`. Additionally for a mutable borrow, the replacement
/// only happens when *every* use of `_2` after it is bound is a `*_2`.
///
/// Now during codegen, each place carries the pointer, the type, and a separate align value.
/// `_2` was a raw pointer `*const T`. When reading and writing to a raw pointer through the deref
/// place `*p`, Rustlang requires it be aligned for T already, which is the promise we made,
/// so when it sees a deref `*_2`, align is 4. Which is why `copy (*_2)` lowers to `ldr`.
///
/// *But* once the ReferencePropagation happens, suddenly the connection back to the struct is made,
/// because `_2` is not just "any aligned T" anymore but replaced with `(*_1).1`, which is "field `.1`
/// of a packed struct pointed to by `_1` where the inherited alignment should thus be 1".
///
/// This can be fixed with an aligned wrapper, since the newtype changes the MIR to:
///
/// ```
/// _2 = &raw const ((*_1).1: u32);
/// _3 = copy _2 as *const A4<u32> (PtrToPtr);
/// _0 = copy ((*_3).0: u32);
/// ```
///
/// `_3` is a pointer to a struct with alignment for T, so the `*_3` reports an align of 4, and it
/// finally lowers to just `ldr`. ReferencePropagation also has no way to replace `(*_3).0` with a `(*_1).1`,
/// nor replace it with `_2` since that would be a backwards optimization that ignores struct
/// information (it would have to be a `reverse-ReferencePropagation` optimization, which doesn't make sense).
///
/// In `-O0`, all of this is fine and it works either way since the MIR ReferencePropagation optimization
/// doesn't run, and in fact MIR inlining also doesn't happen so that optimization would also not do much.
/// But we want it to work on optimized builds.
macro_rules! aligned_wrappers {
    ($($name:ident = $n:literal),* $(,)?) => {
        $(
            #[derive(Clone, Copy)]
            #[repr(C, align($n))]
            pub struct $name<T>(pub T);
        )*

        /// `src` must be aligned for `T`.
        #[inline(always)]
        unsafe fn read_promised<T>(src: *const T) -> T {
            $(
                if const { core::mem::align_of::<T>() == $n } {
                    return unsafe { core::intrinsics::read_via_copy(src as *const $name<T>).0 };
                }
            )*
            unsafe { core::intrinsics::read_via_copy(src) }
        }

        /// `dst` must be aligned for `T`.
        #[inline(always)]
        unsafe fn write_promised<T>(dst: *mut T, val: T) {
            $(
                if const { core::mem::align_of::<T>() == $n } {
                    return unsafe { core::intrinsics::write_via_move(dst as *mut $name<T>, $name(val)) };
                }
            )*
            unsafe { core::intrinsics::write_via_move(dst, val) }
        }
    };
}

aligned_wrappers! {
    A1 = 1, A2 = 2, A4 = 4, A8 = 8, A16 = 16, A32 = 32, A64 = 64, A128 = 128,
    A256 = 256, A512 = 512, A1024 = 1024, A2048 = 2048, A4096 = 4096,
    A8192 = 8192, A16384 = 16384, A32768 = 32768, A65536 = 65536,
    A131072 = 131072, A262144 = 262144, A524288 = 524288,
    A1048576 = 1048576, A2097152 = 2097152, A4194304 = 4194304,
    A8388608 = 8388608, A16777216 = 16777216, A33554432 = 33554432,
    A67108864 = 67108864, A134217728 = 134217728, A268435456 = 268435456,
    A536870912 = 536870912,
}

pub trait RWT: Copy {
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
        unsafe { read_promised(src) }
    }

    #[inline(always)]
    unsafe fn write<T: Copy>(dst: *mut T, val: T) -> T {
        unsafe { write_promised(dst, val) }
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
    pub const fn as_ptr(&self) -> *const T {
        self.ptr
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
    pub const fn as_ptr(&self) -> *mut T {
        self.ptr
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

impl<T, const N: usize, RW: RWT> Volatile<[T; N], RW> {
    #[inline(always)]
    pub const fn len(&self) -> usize {
        N
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        N == 0
    }
}

impl<T, const N: usize, RW: RWT> VolatileMut<[T; N], RW> {
    #[inline(always)]
    pub const fn len(&self) -> usize {
        N
    }

    #[inline(always)]
    pub const fn is_empty(&self) -> bool {
        N == 0
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
/// in a volatile wrapper of type `E::IntoVolatile`, with read-write using provided `RW`
#[derive(Clone, Copy)]
pub struct Vol<RW = VolatileRW>(PhantomData<RW>);

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

impl<E: IntoVolatile, RW: RWT> VolWrap<E> for Vol<RW>
where
    E::IntoVolatile: WithRW,
{
    type Output = <E::IntoVolatile as WithRW>::WithRW<RW>;

    #[inline(always)]
    fn wrap(e: E) -> Self::Output {
        WithRW::with_rw::<RW>(e.into_volatile())
    }
}

pub trait WithRW {
    type WithRW<R: RWT>;
    fn with_rw<R: RWT>(self) -> Self::WithRW<R>;
}

impl<T: Copy, RW: RWT> WithRW for Volatile<T, RW> {
    type WithRW<R: RWT> = Volatile<T, R>;
    #[inline(always)]
    fn with_rw<R: RWT>(self) -> Volatile<T, R> {
        Volatile::<T, RW>::with_rw::<R>(self)
    }
}

impl<T: Copy, RW: RWT> WithRW for VolatileMut<T, RW> {
    type WithRW<R: RWT> = VolatileMut<T, R>;
    #[inline(always)]
    fn with_rw<R: RWT>(self) -> VolatileMut<T, R> {
        VolatileMut::<T, RW>::with_rw::<R>(self)
    }
}

impl<E, RW: RWT> WithRW for ElemPtrs<E, Vol<RW>> {
    type WithRW<R: RWT> = ElemPtrs<E, Vol<R>>;
    #[inline(always)]
    fn with_rw<R: RWT>(self) -> ElemPtrs<E, Vol<R>> {
        ElemPtrs { base: self.base, range: self.range, init_start: self.init_start, init_len: self.init_len, _w: PhantomData }
    }
}

/// Yields `start + i * size_of::<T>()` for every `i` in the range.
#[derive(Clone)]
pub struct ElemPtrs<E, W = Raw> {
    base: E,
    range: Range<usize>,
    init_start: usize,
    init_len: usize,
    _w: PhantomData<W>,
}

impl<E> ElemPtrs<E, Raw> {
    #[inline(always)]
    fn new(base: E, range: Range<usize>) -> Self {
        let init_len = if range.start < range.end {
            range.end.wrapping_sub(range.start)
        } else {
            0
        };
        Self { base, init_start: range.start, init_len, range, _w: PhantomData }
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

    #[inline(always)]
    #[track_caller]
    pub fn index(&self, idx: usize) -> W::Output {
        KnownLen(self.init_len).check_index(idx);
        W::wrap(self.base.wrapping_add_(self.init_start.wrapping_add(idx)))
    }

    #[inline(always)]
    pub fn as_ptr(&self) -> E {
        self.base.wrapping_add_(self.init_start)
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
        ElemPtrs { base: self.base, range: self.range, init_start: self.init_start, init_len: self.init_len, _w: PhantomData }
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

    // Convert `final` modifiers into read-write types
    (@rw) => { $crate::VolatileRW };
    (@rw !) => { $crate::RegularRW };
    (@rw +) => { $crate::UnalignedVolatileRW };
    (@rw ! +) => { $crate::UnalignedRW };
    (@rw + !) => { $crate::UnalignedRW };

    // Wrap whatever the expression, in into_volatile with the indicated read-write type
    (@wrap [$($m:tt)*] $e:expr) => {
        $crate::WithRW::with_rw::<$crate::raw_field!(@rw $($m)*)>($crate::IntoVolatile::into_volatile($e))
    };

    // Everything that can appear after a `final`
    (@final [$($m:tt)*] $base:expr,) => {
        $crate::raw_field!(@wrap [$($m)*] $base)
    };
    (@final [$($m:tt)*] $base:expr, mut $field:ident $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
    (@final [$($m:tt)*] $base:expr, ldr $(!)? $(+)? $($field:ident)? $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
    (@final [$($m:tt)*] $base:expr, ldr + ! $($field:ident)? $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
    (@final [$($m:tt)*] $base:expr, $($field:ident)? $([$idx:expr])?~, $($rest:tt)+) => {
        compile_error!("`final` is only allowed on the last access of a chain")
    };
    (@final [$($m:tt)*] $base:expr, $($access:tt)+) => {
        $crate::raw_field!(@wrap [$($m)*] $crate::raw_field!($base, $($access)+))
    };

    // Everything that can appear after an `ldr`
    (@ldr [$($m:tt)*] $base:expr, [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!(@ldr [$($m)*] $base, [$idx]), $($rest)+)
    };
    (@ldr [$($m:tt)*] $base:expr, [$idx:expr]) => {{
        if false { $crate::__ldr_index(&$idx); }
        let ptr = $crate::raw_field!($base, [$idx]);
        #[allow(unused_unsafe)] unsafe { <$crate::raw_field!(@rw $($m)*) as $crate::RWT>::read(ptr) }
    }};
    (@ldr [$($m:tt)*] $base:expr, $field:ident [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!(@ldr [$($m)*] $base, $field [$idx]), $($rest)+)
    };
    (@ldr [$($m:tt)*] $base:expr, $field:ident~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!(@ldr [$($m)*] $base, $field), $($rest)+)
    };
    (@ldr [$($m:tt)*] $base:expr, $field:ident [$idx:expr]) => {{
        if false { $crate::__ldr_index(&$idx); }
        let ptr = $crate::raw_field!($base, $field [$idx]);
        #[allow(unused_unsafe)] unsafe { <$crate::raw_field!(@rw $($m)*) as $crate::RWT>::read(ptr) }
    }};
    (@ldr [$($m:tt)*] $base:expr, $field:ident) => {{
        let ptr = $crate::raw_field!($base, $field);
        #[allow(unused_unsafe)] unsafe { <$crate::raw_field!(@rw $($m)*) as $crate::RWT>::read(ptr) }
    }};

    // Extract just `final` and its modifiers. These have to come before every `$field:ident` arm,
    // since `ident` also matches keywords like `final`.
    ($base:expr, final ! + $($rest:tt)*) => { $crate::raw_field!(@final [! +] $base, $($rest)*) };
    ($base:expr, final + ! $($rest:tt)*) => { $crate::raw_field!(@final [+ !] $base, $($rest)*) };
    ($base:expr, final ! $($rest:tt)*) => { $crate::raw_field!(@final [!] $base, $($rest)*) };
    ($base:expr, final + $($rest:tt)*) => { $crate::raw_field!(@final [+] $base, $($rest)*) };
    ($base:expr, final $($rest:tt)*) => { $crate::raw_field!(@final [] $base, $($rest)*) };

    // Extract just `ldr` and its modifiers.
    ($base:expr, ldr ! + $($rest:tt)+) => { $crate::raw_field!(@ldr [! +] $base, $($rest)+) };
    ($base:expr, ldr + ! $($rest:tt)+) => { $crate::raw_field!(@ldr [+ !] $base, $($rest)+) };
    ($base:expr, ldr ! $($rest:tt)+) => { $crate::raw_field!(@ldr [!] $base, $($rest)+) };
    ($base:expr, ldr + $($rest:tt)+) => { $crate::raw_field!(@ldr [+] $base, $($rest)+) };
    ($base:expr, ldr $field:ident $($rest:tt)*) => { $crate::raw_field!(@ldr [] $base, $field $($rest)*) };
    ($base:expr, ldr [$idx:expr] $($rest:tt)*) => { $crate::raw_field!(@ldr [] $base, [$idx] $($rest)*) };

    ($base:expr, mut [$idx:expr] $($rest:tt)*) => {
        compile_error!("bare indexing does not take `mut` because mutability comes from the previous raw pointer itself")
    };
    ($base:expr, [$idx:expr]) => {
        $crate::raw_field!(@index $idx, $base)
    };
    ($base:expr, mut $field:ident [$idx:expr]) => {{
        #[allow(unused_unsafe)] unsafe { $crate::raw_field!(@index $idx, &raw mut (*($base)).$field) }
    }};
    ($base:expr, $field:ident [$idx:expr]) => {{
        #[allow(unused_unsafe)] unsafe { $crate::raw_field!(@index $idx, &raw const (*($base)).$field) }
    }};

    ($base:expr, mut $field:ident) => {{
        #[allow(unused_unsafe)] unsafe { &raw mut (*($base)).$field }
    }};
    ($base:expr, $field:ident) => {{
        #[allow(unused_unsafe)] unsafe { &raw const (*($base)).$field }
    }};

    // Chaining arms: peel off the first access (which must not be `final` or a range)
    // and then feed its pointer back in as the base of the rest.
    //
    // We can have a range right before a standalone `final`, so these two arms deliberately skip the
    // single-index check when `final` comes next. They *have* to come before the general ones below.
    ($base:expr, [$idx:expr]~, final $($rest:tt)*) => {
        $crate::raw_field!($crate::raw_field!($base, [$idx]), final $($rest)*)
    };
    ($base:expr, [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!({
            if false { $crate::__single_index(&$idx); }
            $crate::raw_field!($base, [$idx])
        }, $($rest)+)
    };
    ($base:expr, mut $field:ident [$idx:expr]~, final $($rest:tt)*) => {
        $crate::raw_field!($crate::raw_field!($base, mut $field [$idx]), final $($rest)*)
    };
    ($base:expr, $field:ident [$idx:expr]~, final $($rest:tt)*) => {
        $crate::raw_field!($crate::raw_field!($base, $field [$idx]), final $($rest)*)
    };
    ($base:expr, mut $field:ident [$idx:expr]~, $($rest:tt)+) => {
        $crate::raw_field!({
            if false { $crate::__single_index(&$idx); }
            $crate::raw_field!($base, mut $field [$idx])
        }, $($rest)+)
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
    ($base:expr, $field:ident~, $($rest:tt)+) => {
        $crate::raw_field!($crate::raw_field!($base, $field), $($rest)+)
    };
}

