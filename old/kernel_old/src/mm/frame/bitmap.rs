use core::mem;
use core::slice;

/// The number of bits in each entry of the backing slice
const BITS_PER_ITEM: usize = mem::size_of::<usize>() * 8;

#[derive(Debug)]
pub struct Bitmap<'a> {
    contents: &'a mut [usize],
}

impl<'a> Bitmap<'a> {
    pub fn new(backing: &'a mut [usize]) -> Bitmap<'a> {
        Bitmap { contents: backing }
    }

    /// Creates a bitmap backed by an arbitraty memory location. Note that `len` is the length of
    /// the array starting at `ptr` in `usize`s, _not_ bits.
    pub unsafe fn from_raw_parts(ptr: *mut usize, len: usize) -> Bitmap<'a> {
        Bitmap::new(slice::from_raw_parts_mut(ptr, len))
    }

    /// Returns `true` if the given bit is set
    pub fn is_set(&self, bit: usize) -> bool {
        debug_assert!(
            bit < self.contents.len() * BITS_PER_ITEM,
            "Bit {} is out of bounds",
            bit
        );
        self.contents[bit / BITS_PER_ITEM] & (1 << (bit % BITS_PER_ITEM)) != 0
    }

    /// Sets the given bit
    pub fn set(&mut self, bit: usize) {
        debug_assert!(
            bit < self.contents.len() * BITS_PER_ITEM,
            "Bit {} is out of bounds",
            bit
        );
        self.contents[bit / BITS_PER_ITEM] |= 1 << (bit % BITS_PER_ITEM)
    }

    /// Clears the given bit
    pub fn clear(&mut self, bit: usize) {
        debug_assert!(
            bit < self.contents.len() * BITS_PER_ITEM,
            "Bit {} is out of bounds",
            bit
        );
        self.contents[bit / BITS_PER_ITEM] &= !(1 << (bit % BITS_PER_ITEM))
    }

    /// Flips the state of the given bit
    pub fn flip(&mut self, bit: usize) {
        debug_assert!(
            bit < self.contents.len() * BITS_PER_ITEM,
            "Bit {} is out of bounds",
            bit
        );
        self.contents[bit / BITS_PER_ITEM] ^= 1 << (bit % BITS_PER_ITEM)
    }
}
