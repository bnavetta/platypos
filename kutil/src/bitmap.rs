use core::slice;

/// A fixed-size bitmap
pub struct Bitmap<'a> {
    data: &'a mut [u8],
}

impl<'a> Bitmap<'a> {
    pub fn from_slice(data: &'a mut [u8]) -> Bitmap<'a> {
        Bitmap { data }
    }

    /// Create a bitmap from a pointer and a length. The `len` argument is the size
    /// in bytes, not bits, of the bitmap
    pub unsafe fn from_raw_parts(data: *mut u8, len: usize) -> Bitmap<'a> {
        Bitmap {
            data: slice::from_raw_parts_mut(data, len),
        }
    }

    pub fn len(&self) -> usize {
        self.data.len() * 8
    }

    pub fn is_set(&self, index: usize) -> bool {
        let byte = self.data[index / 8];
        byte >> (index % 8) & 0x1 != 0
    }

    pub fn set(&mut self, index: usize) {
        let byte = &mut self.data[index / 8];
        *byte |= 1 << (index % 8);
    }

    pub fn clear(&mut self, index: usize) {
        let byte = &mut self.data[index / 8];
        *byte &= !(1 << (index % 8));
    }
}

#[cfg(test)]
mod tests {
    use super::Bitmap;

    #[test]
    fn test_creation() {
        let mut storage = [0; 4];
        let b = Bitmap::from_slice(&mut storage);
        assert_eq!(b.len(), 32);
    }

    #[test]
    fn test_operations() {
        let mut storage = [0; 4];
        let mut b = Bitmap::from_slice(&mut storage);

        for i in 0..b.len() {
            assert_eq!(b.is_set(i), false);
            b.set(i);
            assert_eq!(b.is_set(i), true);
            b.clear(i);
            assert_eq!(b.is_set(i), false);
        }
    }
}
