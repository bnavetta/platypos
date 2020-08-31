//! Bitmap implementation. This doesn't _really_ belong in the `mem` package, but it's here for now
//! since it's used for the page frame allocator.

// TODO: consider using https://github.com/phil-opp/rust-bit-field

/// Storage unit type for the bitmap
type Storage = u64;

/// Number of bits in a unit of storage
const STORAGE_BITS: usize = 64;

/// A bitmap which does not own its backing storage.
pub struct Bitmap<'a> {
    storage: &'a mut [Storage]
}

impl <'a> Bitmap<'a> {
    pub fn new(storage: &'a mut[Storage]) -> Bitmap<'a> {
        Bitmap { storage }
    }

    /// Returns whether or not a given bit is set.
    ///
    /// # Panics
    /// If `idx` is out of bounds
    #[inline]
    pub fn get(&self, idx: usize) -> bool {
        if let Some(block) = self.storage.get(idx / STORAGE_BITS) {
            let mask = 1 << (idx % STORAGE_BITS);
            block & mask != 0
        } else {
            panic!("Index out of bounds: index is {}, but length is {}", idx, self.len())
        }
    }

    /// Sets or clears the given bit, based on `value`.
    ///
    /// # Panics
    /// If `idx` is out of bounds
    #[inline]
    pub fn set(&mut self, idx: usize, value: bool) {
        if let Some(block) = self.storage.get_mut(idx / STORAGE_BITS) {
            let mask = 1 << (idx % STORAGE_BITS);
            if value {
                *block |= mask;
            } else {
                *block &= !mask;
            }
        } else {
            panic!("Index out of bounds: index is {}, but length is {}", idx, self.len())
        }
    }

    /// Size of this bitmap
    pub fn len(&self) -> usize {
        self.storage.len() * STORAGE_BITS
    }
}
