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
        start SomeValuesViewOnly // can't be instantiated
        start* SomeValues // can be instantiated
            a: u32,
            b: u32,
            stop SomeValuesViewOnly
            stop SomeValues
        start* _Empty
            _empty: PhantomData<()>,
            stop _Empty
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
unsafe fn sum<I: In<S = Example>>(p: *const I) -> u32 {
    unsafe { imply!(p: *const I => Example::SomeValues { (*p).a + (*p).b } do_something(); do_something_else(); 0) }
}
```

