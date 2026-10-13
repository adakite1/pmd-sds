Some notes on these crates:

- "The correctness of code should not suddenly change because of a cfg"
- "Code should not go between not correct (no compile errors) and correct just because a cfg changed"
- Not relying on cfgs for type checking functions

- Instantiation is separated from view, since which type is instantiated determines the memory layout considering alignment of the types for fields. View doesn't determine memory layout in the same way, a view consumes the layout that is already decided.

- In Rustlang, there's no way to express that there are multiple structs who: 1) share every field definition, but 2) does not necessarily share a memory layout. Without making it impossible to address it as a single type that gets type-checked, anyways.

- The two extremes are using function parameters and passing memory and then manually ensuring that every value means the same thing semantically (like dma_array as an Option, which also messes up codegen)... and passing a const list of bitflags as a u64 or some other numeric type and to keep track of the connection between those abstract bitflags and the things they allow yourself with plenty of raw pointer casting.
This is a middle ground, and at the most reduced form you just have definitions where it is empty in one context and non-empty in another (see Example below), so just one regular span and one empty span for "this piece of memory does not exist".
So you still get *functionality* tied to the *memory it uses and if that exists or not* instead of arbitrary flags, and you still guarantee const branch dropping instead of making that an actual branch like in the semantic extreme (`Option<Something>` parameters with defined meanings and rules to make it work as generically as promised) where there is no such guarantee, and where the compiler would almost certainly not monomorphize either, leaving branches in.

Example:

```rust
many_spans! {
    #[repr(C)]
    pub struct Example {
        +SomeValuesViewOnly // can't be instantiated
        +SomeValues* // can be instantiated
            a: u32,
            b: u32,
        SomeValuesViewOnly
        SomeValues
        +_Empty*
            _empty: PhantomData<()>,
        _Empty
    }
}

#[cfg(feature = "values")]
static mut EXAMPLE: Example!(SomeValues) = /* ... */;
#[cfg(not(feature = "values"))]
static mut EXAMPLE: Example!(_Empty) = /* ... */;

/// This compiles to:
///
/// ```rust
///     do_something();
///     do_something_else();
///     0
/// ```
///
///  if this is called with #[cfg(not(feature = "values"))], where p is still *const I,
///
/// and it compiles to:
///
/// ```rust
///     (*p).a + (*p).b
/// ```
///
///  if this is called with #[cfg(feature = "values")], and in that case, p becomes properly typed inside those braces.
///
/// But either way, the function is still type-checked.
/// None of the branches are greyed out like if you used cfgs the usual way,
/// but neither are you keeping track of if some bitflag F.get(0) or whatever
/// let you use some address EXAMPLE_INSTANCE or if that wasn't the case or
/// not, and you are also guaranteed to get separate codegen instead of
/// one function with a runtime branch.
///
/// In practice you'd probably want to just name EXAMPLE directly and use the raw pointer,
/// but this is an example to show you can do this too and expose some of the internals. To
/// do that, you'll need to either make this `inline(always)` and call it from a one-line wrapper
/// functions, or cfg the types ("type ExampleType = Example!(SomeValues) or Example!(_Empty)"),
/// or use `guard!` instead of `imply!` (`guard!` does the `inline(always)` inner and one-line
/// wrapper outer call under-the-hood).
fn sum<I: In<S = Example>>(p: *const I) -> u32 {
    imply!(p: *const I => Example::SomeValues { (*p).a + (*p).b } do_something(); do_something_else(); 0)
}
```

Just to be *EXTRA* clear about why this is good, here is an example:

```rust
#[repr(C)] pub struct Processor1Table { pub hardware_1: [Cb; 4], pub hardware_2: [Cb; 4] }
#[repr(C)] pub struct Processor2Table { pub hardware_2: [Cb; 4], pub hardware_3: [Cb; 1] }

#[cfg(feature = "processor1")]
static mut TABLE: Processor1Table = Processor1Table { hardware_1: [EMPTY; 4], hardware_2: [EMPTY; 4] };
#[cfg(feature = "processor2")]
static mut TABLE: Processor2Table = Processor2Table { hardware_2: [EMPTY; 4], hardware_3: [EMPTY; 1] };

#[inline(always)]
fn enable_impl<const HAS_HARDWARE_1: bool, const HAS_HARDWARE_3: bool>( // <== BAD, you have to keep track of which booleans mean what and there is absolutely no type tracking. If you forget to do an if const somewhere, the asm looks fine on one cfg, and then on the other you have a bad access.
    hardware_2: *mut [Cb; 4], // <== BAD, these could be null and you wouldn't know it at all from just looking at this
    hardware_1: *mut [Cb; 4],
    hardware_3: *mut [Cb; 1],
) {
    unsafe {
        (*hardware_2)[0].enable = 1;

        if const { HAS_HARDWARE_1 } {
            (*hardware_1)[0].enable = 1;
        }
        if const { HAS_HARDWARE_3 } {
            (*hardware_3)[0].enable = 1;
        }
    }
}

#[cfg(feature = "processor1")]
pub fn enable() {
    let t = &raw mut TABLE;
    unsafe {
        enable_impl::<true, false>( // <== BAD, what does "<true, false>" even mean??!?
            &raw mut (*t).hardware_2,
            &raw mut (*t).hardware_1,
            core::ptr::dangling_mut(),
        );
    }
}

#[cfg(feature = "processor2")] // <== BAD, the entire block is greyed out until you decide to switch cfg's, and then when you switch, check, fix, and come back, the other block is erroring now
pub fn enable() {
    let t = &raw mut TABLE;
    unsafe {
        enable_impl::<false, true>(
            &raw mut (*t).hardware_2,
            core::ptr::dangling_mut(), // <== BAD, I hope you remember the order was hardware_2, hardware_1, then hardware_3, and the dangling_mut is standing in for the hardware_1
            &raw mut (*t).hardware_3,
        );
    }
}



/// NOW THE GOOD VERSION:



use many_spans::{imply, many_spans};

many_spans! {
    #[repr(C)]
    pub struct Table_ {
        +P1*
            pub hardware_1: [Cb; 4],
        +P2*
        +Hardware2
            pub hardware_2: [Cb; 4],
        Hardware2
        P1
            pub hardware_3: [Cb; 1],
        P2
    }
}

#[cfg(feature = "processor1")]
type Table = Table_!(P1);
#[cfg(feature = "processor2")]
type Table = Table_!(P2);

#[cfg(feature = "processor1")]
static mut TABLE: Table = Table { hardware_1: [EMPTY; 4], hardware_2: [EMPTY; 4] };
#[cfg(feature = "processor2")]
static mut TABLE: Table = Table { hardware_2: [EMPTY; 4], hardware_3: [EMPTY; 1] };

pub fn enable() {
    let t = &raw mut TABLE;

    unsafe {
        (*t).hardware_2[0].enable = 1;

        imply!(t: *mut Table => Table_::P1 {
            (*t).hardware_1[0].enable = 1;
        });
        imply!(t: *mut Table => Table_::P2 {
            (*t).hardware_3[0].enable = 1;
        });
    }
}



// You see now
```

