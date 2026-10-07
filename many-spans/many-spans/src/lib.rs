// SPDX-License-Identifier: MIT OR Apache-2.0
// Copyright (c) 2026 adakite1
//
// Licensed under either of Apache License, Version 2.0 (http://www.apache.org/licenses/LICENSE-2.0)
// or the MIT license (http://opensource.org) at your option.

#![no_std]

pub use many_spans_macros::many_spans;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct Span {
    pub id: usize,
    pub first: usize,
    pub end: usize,
    pub offset: usize,
    pub size: usize,
}

impl Span {
    pub const fn field_count(self) -> usize {
        self.end - self.first
    }

    pub const fn serves(self, target: Span) -> bool {
        self.first <= target.first
            && target.end <= self.end
            && self.offset <= target.offset
            && target.offset + target.size <= self.offset + self.size
    }

    pub const fn delta_to(self, target: Span) -> usize {
        if self.serves(target) {
            target.offset - self.offset
        } else {
            0
        }
    }
}

pub unsafe trait SpanTable {
    const SPANS: &'static [Span];

    const NAMES: &'static [&'static str];
}

pub unsafe trait HasSpan<const S: u128>: SpanTable {
    const SPAN: Span;
}

pub unsafe trait HasView<const S: u128>: HasSpan<S> {
    type View;
}

pub unsafe trait SpanInstantiation: Sized {
    type Full: SpanTable + ?Sized;
    const SPAN_ID: u128;
    const SPAN: Span;
    const OFFSETS: &'static [Option<usize>];
}

pub unsafe trait HasInstantiation<const S: u128>: HasSpan<S> {
    type Instantiation: SpanInstantiation<Full = Self>;
}

pub trait In: SpanInstantiation<Full = Self::S> {
    type S: SpanTable + ?Sized;
}

impl<I: SpanInstantiation> In for I {
    type S = I::Full;
}

pub type InstantiationOf<F, const S: u128> = <F as HasInstantiation<S>>::Instantiation;

pub type ViewOf<F, const S: u128> = <F as HasView<S>>::View;

pub const fn span<F: HasSpan<S> + ?Sized, const S: u128>() -> Span {
    F::SPAN
}

pub const fn span_of<S: SpanInstantiation>(_: *const S) -> Span {
    S::SPAN
}

pub const fn name_of<S: SpanInstantiation>(_: *const S) -> &'static str {
    <S::Full as SpanTable>::NAMES[S::SPAN.id]
}

pub const fn serves<S: SpanInstantiation, const T: u128>() -> bool
where
    S::Full: HasSpan<T>,
{
    S::SPAN.serves(<S::Full as HasSpan<T>>::SPAN)
}

#[inline(always)]
pub const unsafe fn view<S: SpanInstantiation, const T: u128>(
    p: *const S,
) -> *const ViewOf<S::Full, T>
where
    S::Full: HasView<T>,
{
    unsafe { p.byte_add(const { S::SPAN.delta_to(<S::Full as HasSpan<T>>::SPAN) }).cast() }
}

#[inline(always)]
pub const unsafe fn view_mut<S: SpanInstantiation, const T: u128>(
    p: *mut S,
) -> *mut ViewOf<S::Full, T>
where
    S::Full: HasView<T>,
{
    unsafe { p.byte_add(const { S::SPAN.delta_to(<S::Full as HasSpan<T>>::SPAN) }).cast() }
}

#[macro_export]
macro_rules! imply {
    // ($v:ident : *const $t:ty => $span:path $body:block) => {
    //     ::core::compile_error!("must have something if imply does not hold")
    // };
    // ($v:ident : *mut $t:ty => $span:path $body:block) => {
    //     ::core::compile_error!("must have something if imply does not hold")
    // };
    ($v:ident : *const $t:ty => $span:path $body:block $($else:tt)*) => {
        if const { $crate::serves::<$t, { $span }>() } {
            let $v = $crate::view::<$t, { $span }>($v);
            $body
        } else {
            $($else)*
        }
    };
    ($v:ident : *mut $t:ty => $span:path $body:block $($else:tt)*) => {
        if const { $crate::serves::<$t, { $span }>() } {
            let $v = $crate::view_mut::<$t, { $span }>($v);
            $body
        } else {
            $($else)*
        }
    };
}

#[macro_export]
macro_rules! guard {
    ($v:ident if $($full:ident)::+ with $span:ident => $ret:ty $body:block $($else:tt)*) => {
        $crate::guard!(@expand [*const] [view] ($v = $v) [] [$($full)::+] $span; $ret; $body; $($else)*)
    };
    ($v:ident mut if $($full:ident)::+ with $span:ident => $ret:ty $body:block $($else:tt)*) => {
        $crate::guard!(@expand [*mut] [view_mut] ($v = $v) [] [$($full)::+] $span; $ret; $body; $($else)*)
    };
    (($name:ident = $e:expr $(, $($b:ident)+ : $t:ty = $x:expr)* $(,)?) if $($full:ident)::+ with $span:ident => $ret:ty $body:block $($else:tt)*) => {
        $crate::guard!(@expand [*const] [view] ($name = $e) [$($($b)+ : $t = $x),*] [$($full)::+] $span; $ret; $body; $($else)*)
    };
    (($name:ident = $e:expr $(, $($b:ident)+ : $t:ty = $x:expr)* $(,)?) mut if $($full:ident)::+ with $span:ident => $ret:ty $body:block $($else:tt)*) => {
        $crate::guard!(@expand [*mut] [view_mut] ($name = $e) [$($($b)+ : $t = $x),*] [$($full)::+] $span; $ret; $body; $($else)*)
    };
    (($name:ident $(, $($b:ident)+ : $t:ty = $x:expr)* $(,)?) if $($full:ident)::+ with $span:ident => $ret:ty $body:block $($else:tt)*) => {
        $crate::guard!(@expand [*const] [view] ($name = $name) [$($($b)+ : $t = $x),*] [$($full)::+] $span; $ret; $body; $($else)*)
    };
    (($name:ident $(, $($b:ident)+ : $t:ty = $x:expr)* $(,)?) mut if $($full:ident)::+ with $span:ident => $ret:ty $body:block $($else:tt)*) => {
        $crate::guard!(@expand [*mut] [view_mut] ($name = $name) [$($($b)+ : $t = $x),*] [$($full)::+] $span; $ret; $body; $($else)*)
    };
    (@expand [$($ptr:tt)+] [$view:ident] ($name:ident = $e:expr) [$($($b:ident)+ : $t:ty = $x:expr),*] [$($full:ident)::+] $span:ident; $ret:ty; $body:block; $($else:tt)*) => {{
        #[inline(always)]
        unsafe fn __call<I: $crate::In<S = $($full)::+>>($name: $($ptr)+ I $(, $($b)+ : $t)*) -> $ret {
            if const { $crate::serves::<I, { $($full)::+::$span }>() } {
                let $name = unsafe { $crate::$view::<I, { $($full)::+::$span }>($name) };
                $body
            } else {
                $($else)*
            }
        }
        __call($e $(, $x)*)
    }};
}

#[cfg(test)]
mod tests {
    use super::Span;

    const A: Span = Span {
        id: 0,
        first: 0,
        end: 2,
        offset: 0,
        size: 16,
    };
    const B: Span = Span {
        id: 1,
        first: 1,
        end: 3,
        offset: 8,
        size: 12,
    };
    const C: Span = Span {
        id: 2,
        first: 1,
        end: 2,
        offset: 8,
        size: 8,
    };

    #[test]
    fn containment() {
        assert!(A.serves(A) && A.serves(C) && B.serves(C));
        assert!(!A.serves(B) && !B.serves(A) && !C.serves(A));
    }

    #[test]
    fn deltas() {
        assert_eq!(A.delta_to(C), 8);
        assert_eq!(B.delta_to(C), 0);
        assert_eq!(C.delta_to(A), 0);
    }
}

