use core::mem;

/// Computes the integer part of the base-2 logarithm of x
pub const fn log2(x: usize) -> usize {
    // https://en.wikipedia.org/wiki/Find_first_set
    (mem::size_of::<usize>() * 8) - 1 - (x.leading_zeros() as usize)
}
