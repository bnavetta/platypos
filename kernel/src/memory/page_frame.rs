use core::mem;
use core::ptr;

use x86_64::{PhysAddr};
use x86_64::structures::paging::{PhysFrame, PhysFrameRange};

/// The number of orders (distinct block sizes) supported. A block of order `i` contains
/// `2^i` contiguous page frames.
const ORDERS: usize = 8;

/// A contiguous region of memory we can allocate from
///
/// In memory, the region is laid out with the `FrameGroup` struct at the very end, with the
/// bitmaps for each order before it.
///
pub struct FrameGroup {
    /// First page frame in the group
    start: PhysFrame,

    /// Number of page frames in the group
    size: usize,

    // For each order, a list of free blocks of that order, represented as a reference to
    // the first free block in the list
    free_lists: [Option<&'static FreeBlock>; ORDERS],
    maps: [Bitmap; ORDERS],
}

impl FrameGroup {
    pub fn create(range: PhysFrameRange) -> &'static mut FrameGroup {
        let mut end = range.end.start_address() + range.end.size();
        let npages = range.start - range.end;

        end -= mem::size_of::<FrameGroup>();
        let group = unsafe { mem::transmute::<u64, &'static mut FrameGroup>(end.as_u64()) };

        group.start = range.start;

        // Initialize the allocation bitmap for each order.
        for i in 1..ORDERS {
            // How many blocks are in this order?
            let count = npages
        }

        group
    }
}

// TODO: stack / linked list abstraction for "embedded" lists like in Weenix

struct FreeBlock {
    prev: Option<&'static FreeBlock>,
    next: Option<&'static FreeBlock>
}

#[repr(transparent)]
struct Bitmap(*mut u64);

/// Very thin wrapper around a region of memory being used as a bitmap. Operations are all unsafe
/// because they do absolutely no bounds checking.
impl Bitmap {
    // For every bit index, the high 58 bits index into the array of u64's and the low 6 bits index
    // into the particular u64.

    /// Given a bit index, determine the block (u64) index and the offset within that block.
    #[inline]
    fn location_of(bit: usize) -> (usize, usize) {
        (bit >> 6, bit & 0x3f)
    }

    unsafe fn flip_bit(&mut self, bit: usize) {
        let (index, offset) = Bitmap::location_of(bit);
        *self.0.offset(index as isize) ^= 1 << offset;
    }

    unsafe fn set_bit(&mut self, bit: usize) {
        let (index, offset) = Bitmap::location_of(bit);
        *self.0.offset(index as isize) |= 1 << offset;
    }

    unsafe fn clear_bit(&mut self, bit: usize) {
        let (index, offset) = Bitmap::location_of(bit);
        *self.0.offset(index as isize) &= !(1 << offset);
    }

    unsafe fn check_bit(&self, bit: usize) -> bool {
        let (index, offset) = Bitmap::location_of(bit);
        *self.0.offset(index as isize) & 1 << offset != 0
    }
}

#[cfg(test)]
mod test {
    use super::Bitmap;

    #[test]
    fn test_bitmap() {
        let mut buf = [0u64; 4];
        let mut bitmap = Bitmap(buf.as_mut_ptr());

        for i in 0..256 {
            unsafe {
                assert_eq!(bitmap.check_bit(i), false);
                bitmap.flip_bit(i);
                assert_eq!(bitmap.check_bit(i), true);
                bitmap.flip_bit(i);
                assert_eq!(bitmap.check_bit(i), false);

                bitmap.set_bit(i);
                assert_eq!(bitmap.check_bit(i), true);
                bitmap.set_bit(i);
                assert_eq!(bitmap.check_bit(i), true);

                bitmap.clear_bit(i);
                assert_eq!(bitmap.check_bit(i), false);
                bitmap.clear_bit(i);
                assert_eq!(bitmap.check_bit(i), false);
            }
        }
    }
}