use core::{cmp::Ordering, fmt, hash::{Hash, Hasher}, mem::MaybeUninit};

#[derive(Clone, Copy)]
#[repr(transparent)]
pub struct Padding<const N: usize>([MaybeUninit<u8>; N]);

impl<const N: usize> PartialEq for Padding<N> {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl<const N: usize> Eq for Padding<N> {}

impl<const N: usize> PartialOrd for Padding<N> {
    fn partial_cmp(&self, _: &Self) -> Option<Ordering> {
        Some(Ordering::Equal)
    }
}

impl<const N: usize> Ord for Padding<N> {
    fn cmp(&self, _: &Self) -> Ordering {
        Ordering::Equal
    }
}

impl<const N: usize> Hash for Padding<N> {
    fn hash<H: Hasher>(&self, _: &mut H) {}
}

impl<const N: usize> fmt::Debug for Padding<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Padding")
    }
}

impl<const N: usize> Default for Padding<N> {
    fn default() -> Self {
        Self([MaybeUninit::uninit(); N])
    }
}

