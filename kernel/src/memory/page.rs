use core::mem;
use core::ops::{Add, Sub};
use kutil::bitmap::Bitmap;
use x86_64::PhysAddr;

/// A size or count measured in 4KiB pages
#[derive(Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone)]
struct Pages(usize);

impl From<usize> for Pages {
    fn from(raw: usize) -> Pages {
        Pages(raw)
    }
}

impl Into<usize> for Pages {
    fn into(self: Pages) -> usize {
        self.0
    }
}

impl From<Bytes> for Pages {
    fn from(bytes: Bytes) -> Pages {
        Pages(bytes.0 / 4096)
    }
}

impl Add<usize> for Pages {
    type Output = Pages;

    fn add(self, offset: usize) -> Pages {
        Pages(self.0 + offset)
    }
}

impl Sub<usize> for Pages {
    type Output = Pages;

    fn sub(self, offset: usize) -> Pages {
        Pages(self.0 - offset)
    }
}

/// A size or count measured in bytes
#[derive(Eq, PartialEq, PartialOrd, Ord, Debug, Copy, Clone)]
struct Bytes(usize);

impl From<usize> for Bytes {
    fn from(raw: usize) -> Bytes {
        Bytes(raw)
    }
}

impl Into<usize> for Bytes {
    fn into(self: Bytes) -> usize {
        self.0
    }
}

impl From<Pages> for Bytes {
    fn from(pages: Pages) -> Bytes {
        Bytes(pages.0 * 4096)
    }
}

#[derive(Copy, Clone, Eq, PartialEq, PartialOrd, Ord, Debug)]
struct Order(u8);

impl Order {
    /// The size of a block of this order
    fn block_size(&self) -> Pages {
        (1usize << self.0).into()
    }

    /// Determines the size of the bitmap for this order in a region
    ///
    /// # Arguments
    /// * `region_size` the size of the region
    fn bitmap_size(&self, region_size: Pages) -> Bytes {
        assert!(region_size >= self.block_size()); // Make sure we actually fit in the region

        // A r-page region can hold r / 2^n block of order n
        // Use bit shifting to divide by 2^n then again by 8 because we only need a bit per block
        Bytes(region_size.0 as usize >> (self.0 + 3))
    }

    /// Determines the offset of the bitmap for this order relative to the start of the bitmap space
    /// in memory
    ///
    /// # Arguments
    /// * `region_size` the size of the region
    fn bitmap_offset(&self, region_size: Pages) -> usize {
        // Cumulative sum of the sizes of all lower orders' bitmaps
        (0..self.0).map::<usize, _>(|o| Order(o).bitmap_size(region_size).into()).sum()
    }
}

impl Add<u8> for Order {
    type Output = Order;

    fn add(self, offset: u8) -> Order {
        Order(self.0 + offset)
    }
}

impl Sub<u8> for Order {
    type Output = Order;

    fn sub(self, offset: u8) -> Order {
        Order(self.0 - offset)
    }
}

struct Region {
    start: PhysAddr,
    npages: Pages,
    max_order: Order
}

impl Region {
    // TODO: when initializing, remember to mark the header and bitmap pages as allocated

    fn start_ptr(&self) -> *mut u8 {
        self.start.as_u64() as *mut u8
    }

    unsafe fn bitmap_start(&self) -> *mut u8 {
        self.start_ptr().offset(mem::size_of::<Region>() as isize)
    }

    fn bitmap(&self, order: Order) -> Bitmap {
        assert!(order <= self.max_order);
        unsafe {
            Bitmap::from_raw_parts(
                self.bitmap_start().offset(order.bitmap_offset(self.npages) as isize),
                order.bitmap_size(self.npages).into())
        }
    }

    fn mark_allocated(&mut self, order: Order, index: usize) {
        self.bitmap(order).set(index)
    }

    fn mark_unallocated(&mut self, order: Order, index: usize) {
        self.bitmap(order).clear(index)
    }

    fn is_allocated(&self, order: Order, index: usize) -> bool {
        self.bitmap(order).is_set(index)
    }
}































//const MAX_ORDER: usize = 16;
//
//struct Region {
//    start: PhysAddr,
//    end: PhysAddr,
//    npages: usize,
//    tree_start: *mut u8,
//    data_start: *mut u8,
//}
//
//impl Region {
//    unsafe fn new(start: PhysAddr, end: PhysAddr) {
//        /*
//
//        For order n:
//        - allocates memory in chunks of 2^n * PAGE_SIZE bytes
//        - tree row is region size / chunk size / 8 bytes
//        - row starts at (2^n - 1) / 8
//
//
//        suppose we have orders 0, 1, 2, 3, 4
//
//        0: 1-page units, npages long
//        1: 2-page units, npages / 2 long
//        2: 4-page units, npages / 4 long
//        3: 8-page units, npages / 8 long, starts at npages / 16
//        4: 16-page units, npages / 16 long, starts at 0
//
//        */
//    }
//
//    fn order_row(&self, order: usize) -> *mut u8 {
//        assert!(order <= MAX_ORDER);
//
//        // This relies on two properties:
//        // 1. We put the maximum order (shortest row) first
//        // 2. S
//
//        unsafe { self.tree_start.offset(((1 << order - 1) / 8) as isize) }
//    }
//
//    /*
//     * For is_free and set_free, the bit we want is in the (idx / 8)'th byte of the row, at
//     * offset (idx % 8). We use fun bit masking to get, set, or clear the bit.
//     */
//
//    fn is_free(&self, idx: usize, order: usize) -> bool {
//        assert!(idx <= self.npages / (1 << order)); // Make sure the index fits within the row
//        let row = self.order_row(order);
//
//        let cell = unsafe { *row.offset((idx / 8) as isize) };
//        cell & (1 << (idx % 8)) != 0
//    }
//
//    fn set_free(&mut self, idx: usize, order: usize, free: bool) {
//        assert!(idx <= self.npages / (1 << order)); // Make sure the index fits within the row
//
//        let row = self.order_row(order);
//        if free {
//            unsafe {
//                *row.offset((idx / 8) as isize) |= 1 << (idx % 8);
//            }
//        } else {
//            unsafe {
//                *row.offset((idx / 8) as isize) &= !(1 << (idx % 8));
//            }
//        }
//    }
//}