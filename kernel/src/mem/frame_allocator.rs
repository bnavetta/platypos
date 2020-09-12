//! Allocator for frames of physical memory.
//!
//! The allocator divides usable physical memory into largely-independent *regions*. Each region is
//! internally contiguous and does not overlap with any other region. The first frame or so of each
//! region holds metadata including allocation bitmaps

use core::mem;

use intrusive_collections::{LinkedList, LinkedListLink, intrusive_adapter};
use spinning_top::Spinlock;

use platypos_pal as pal;
use platypos_pal::mem::PageFrame;

use crate::mem::bitmap::Bitmap;
use core::marker::PhantomData;

/*
 - Buddy bitmap allocator with free linked list
 - Regions are independent (simple: iterate through regions until one allocates successfully)
     - can later prioritize low memory for DMA, etc.
 - orders: order n blocks hold 2^n frames, orders go from 0 to... something

 Allocation:
 - find smallest order whose block size is >= # of needed pages
 - get a block of that order and mark it as allocated
 - recursively split so spare blocks can be used:
     input: a block, requested # of frames s.t. block size / 2 <= requested frames <= block size
     if block size == requested frames:
        don't do anything, block is in use but protected by allocation at higher order
     if block size / 2 == requested frames:
         mark upper half of block as free
     else if requested frames >:
         recurse(upper half of block,

     can probably also make iterative

     see example sketch
 */

pub struct FrameAllocator<P: pal::Platform> {
    regions: LinkedList<RegionAdapter<'static, P>>
}

/// Region header. Allocation bitmaps are stored in memory immediately after this header.
struct Region<P: pal::Platform> {
    link: LinkedListLink,
    inner: Spinlock<RegionInner<P>>,

    /// The size of this region, in frames
    size: usize,
}

intrusive_adapter!(RegionAdapter<'a, P> = &'a Region<P>: Region<P> { link: LinkedListLink } where P: pal::Platform);

/// Mutable part of the region header, requiring synchronization
struct RegionInner<P: pal::Platform> {
    /// Allocation bitmaps. `1` means allocated and `0` means unallocated.
    bitmaps: [Bitmap<'static>; BlockOrder::NUM_ORDERS],
    free_lists: [LinkedList<FreeBlockAdapter<'static, P>>; BlockOrder::NUM_ORDERS],

    _platform: PhantomData<&'static P>
}

/// A block allocation order. Blocks of order `k` contain `2^k` frames.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct BlockOrder(u8);

/// A block of physical memory, defined by its order and its starting page frame
#[derive(Debug, Clone, Eq, PartialEq)]
struct Block<P: pal::Platform> {
    order: BlockOrder,
    start: PageFrame<P>
}

struct FreeBlock<P: pal::Platform> {
    /// Magic number at the start of free blocks to help detect memory corruption/bugs
    magic: u64,
    /// The physical page that this block starts with
    start: PageFrame<P>,
    /// The order of this free block
    order: BlockOrder,
    /// Links this into the free block list
    link: LinkedListLink,
}

intrusive_adapter!(FreeBlockAdapter<'a, P> = &'a FreeBlock<P>: FreeBlock<P> { link: LinkedListLink } where P: pal::Platform);

impl <P: pal::Platform> Region<P> {
    fn header_size() -> usize {
        mem::size_of::<Region<P>>()
    }
}

impl <P: pal::Platform> RegionInner<P> {
    /// Attempts to allocate a block of the given `order`.
    ///
    /// If there are no blocks of `order` available, this will try to split a block of a higher
    /// order. If no blocks of higher orders are available either, returns `None`.
    fn allocate(&mut self, order: BlockOrder) -> Option<Block<P>> {
        if let Some(free_block) = self.free_lists[order.as_usize()].cursor_mut().remove() {
            free_block.validate(order);
            let block = free_block.into_block();
            // Only need to mark the block as allocated, since we already removed it from the free list
            self.set_allocated_bit(&block, true);
            Some(block)
        } else if let Some(big_block) = order.step_up().and_then(|ord| self.allocate(ord)) {
            let (low, high) = big_block.children().unwrap();
            self.set_allocated_bit(&low, true);
            self.make_free(high);
            Some(low)
        }

        None
    }

    /// Frees a previously-allocated block.
    fn free(&mut self, block: Block<P>) {
        let buddy = block.buddy();
        if self.is_allocated(&buddy) {
            // If the buddy is free, we can't combine back into the parent block. Just free this block
            self.make_free(block);
        } else if let Some(parent) = block.parent() {
            // Otherwise, combine back into the parent block
            self.set_allocated_bit(&buddy, true);
            // TODO: better free list abstraction
            unsafe {
                let free_block = self.free_block(&buddy);
                free_block.as_mut().unwrap().validate(buddy.order);
                self.free_lists[block.order.as_usize()].cursor_mut_from_ptr(free_block).remove();
            }
            // Use free recursively so we can combine grandparent blocks and so on
            self.free(parent);
        } else {
            // There's no parent block to combine into, because we're already at the largest order
            self.make_free(block);
        }
    }

    /// Updates the bitmap with whether or not `block` is allocated.
    fn set_allocated_bit(&mut self, block: &Block<P>, allocated: bool) {
        self.bitmaps[block.order.as_usize()].set(block.index(), allocated);
    }

    /// Checks if `block` is allocated
    fn is_allocated(&self, block: &Block<P>) -> bool {
        self.bitmaps[block.order.as_usize()].get(block.index())
    }

    /// Gets a pointer to the free block header for a given block.
    ///
    /// # Safety
    /// The caller is responsible for validating and/or initializing the free block information. This
    /// function makes no guarantees or assumptions about whether or not the block is actually free
    /// or allocated.
    unsafe fn free_block(&self, block: &Block<P>) -> *mut FreeBlock<P> {
        unimplemented!("TODO")
    }

    /// Mark a block as unallocated by updating its bitmap entry and adding it to the free list.
    fn make_free(&mut self, block: Block<P>) {
        debug_assert!(self.is_allocated(&block), "tried to free already-freed block {:?}", block);
        self.set_allocated_bit(&block, false);

        // Manipulating the free list is generally unsafe because the list is stored within the
        // blocks themselves. Here, we're taking previously-allocated memory and making it free, so
        // we can put the block in a well-defined state.
        unsafe {
            let free_block = self.free_block(&block).as_mut().unwrap();
            free_block.initialize(block.order, block.start);
            // Treat the free list like a stack
            self.free_lists[block.order.as_usize()].push_front(free_block as &_);
        }
    }
}

impl BlockOrder {
    /// The smallest valid `BlockOrder`, for blocks of a single frame
    const MIN: BlockOrder = BlockOrder(0);

    /// The largest valid `BlockOrder`, for blocks of 128 frames.
    const MAX: BlockOrder = BlockOrder(7);

    /// The number of orders
    const NUM_ORDERS: usize = BlockOrder::MAX.0 as usize + 1;

    /// The number of frames in a block of this order.
    const fn frames(self) -> usize {
        1 << self.0 as usize
    }

    /// The next order larger than this one (`k + 1`). Returns `None` if already `BlockOrder::MAX`.
    fn step_up(self) -> Option<BlockOrder> {
        if self == BlockOrder::MAX {
            None
        } else {
            Some(BlockOrder(self.0 + 1))
        }
    }

    /// The order immediately below this one (`k - 1`). Returns `None` if already `BlockOrder::MIN`
    fn step_down(self) -> Option<BlockOrder> {
        if self == BlockOrder::MIN {
            None
        } else {
            Some(BlockOrder(self.0 - 1))
        }
    }

    fn as_usize(self) -> usize {
        self.0 as usize
    }
}

impl <P: pal::Platform> Block<P> {
    /// The index of this block (i.e. into bitmaps of its order)
    fn index(&self) -> usize {
        self.start.frame_number() / self.order.frames()
    }

    /// Is this block the low half of its parent?
    fn is_low(&self) -> bool {
        // If the index is even, we're the low half, if it's odd, we're the high half
        self.index() % 2 == 0
    }

    /// The low and high halves of this block. Returns `None` if this block is at the lowest order
    /// and cannot be split
    fn children(&self) -> Option<(Block<P>, Block<P>)> {
        self.order.step_down().map(|order| {
            let low = Block { order, start: self.start };
            let high = Block { order, start: self.start + order.frames() };
            (low, high)
        })
    }

    /// The low half of this block. This will be a block starting at the same address but
    /// of the next order down. Returns `None` if this block is already at the lowest order.
    fn low(&self) -> Option<Block<P>> {
        self.order.step_down().map(|order| Block { order, start: self.start })
    }

    /// The high half of this block. This will be a block starting in the middle of this block and
    /// at the next order down. Returns `None` if this block is already at the lowest order.
    fn high(&self) -> Option<Block<P>> {
        self.order.step_down().map(|order| Block { order, start: self.start + order.frames() })
    }

    /// Gets the buddy of this block
    fn buddy(&self) -> Block<P> {
        let start = if self.is_low() {
            self.start + self.order.frames()
        } else {
            self.start - self.order.frames()
        };
        Block { order: self.order, start }
    }

    /// Gets the parent of this block. If this block is already at the highest order, returns `None`
    fn parent(&self) -> Option<Block<P>> {
        if let Some(order) = self.order.step_up() {
            let start = if self.is_low() {
                self.start
            } else {
                self.start + self.order.frames();
            };

            Some(Block { order, start })
        } else {
            None
        }
    }
}

impl <P: pal::Platform> FreeBlock<P> {
    const MAGIC: u64 = 0xf00f00b12b12;

    /// Attempts to verify that this free block is as expected.
    fn validate(&self, expected_order: BlockOrder) {
        assert_eq!(self.magic, Self::MAGIC, "unexpected free block magic {:#x}", self.magic);
        assert_eq!(self.order, expected_order, "expected free block to have order {:?}, but was {:?}", expected_order, self.order);
        assert!(self.link.is_linked(), "expected free block to be in the free list");
    }

    /// Re-initializes a free block by setting all fields to their expected values. This should be
    /// called when adding a formerly-allocated block to the free list.
    fn initialize(&mut self, order: BlockOrder, start: PageFrame<P>) {
        self.magic = Self::MAGIC;
        self.order = order;
        self.start = start;
        self.link = LinkedListLink::new();
    }

    /// Converts this `FreeBlock` into a `Block` structure. This does *not* update any free lists
    /// or bitmaps.
    fn into_block(self) -> Block<P> {
        Block { start: self.start, order: self.order }
    }
}