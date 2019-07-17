//! Physical memory (frame) allocator
//!
//! The allocator is a buddy allocator backed by free lists instead of bitmaps.

use arr_macro::arr;
use intrusive_collections::{intrusive_adapter, RBTree, RBTreeLink, KeyAdapter};
use intrusive_collections::rbtree::{Cursor, CursorMut};
use log::trace;
use spin::Mutex;

use crate::platform::PhysicalAddress;
use crate::platform::memory::{physical_to_virtual, FRAME_SIZE};
use crate::util::log2;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
struct Order(u8);

impl Order {
    const MAX: Order = Order(11);
    const MIN: Order = Order(0);

    /// The number of orders
    const COUNT: usize = Order::MAX.0 as usize + 1;

    /// Get the smallest order containing at least `frames` frames. If no order is large enough,
    /// returns `None`
    fn for_frames(frames: usize) -> Option<Order> {
        if frames > Order::MAX.frames() {
            None
        } else {
            Some(Order(log2(frames.next_power_of_two()) as u8))
        }
    }

    #[inline]
    const fn frames(&self) -> usize {
        1 << (self.0 as usize)
    }

    #[inline]
    const fn bytes(&self) -> usize {
        self.frames() * FRAME_SIZE
    }

    #[inline]
    fn parent(&self) -> Option<Order> {
        if self < &Order::MAX {
            Some(Order(self.0 + 1))
        } else {
            None
        }
    }

    #[inline]
    fn child(&self) -> Option<Order> {
        if self > &Order::MIN {
            Some(Order(self.0 - 1))
        } else {
            None
        }
    }

    #[inline]
    fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug)]
struct FreeBlock {
    start: PhysicalAddress,
    order: Order,
    link: RBTreeLink
}

intrusive_adapter!(FreeBlockAdapter = &'static FreeBlock : FreeBlock { link: RBTreeLink });

impl <'a> KeyAdapter<'a> for FreeBlockAdapter {
    type Key = PhysicalAddress;

    fn get_key(&self, value: &'a FreeBlock) -> PhysicalAddress {
        value.start
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
struct BlockKey {
    order: Order,
    start: PhysicalAddress
}

impl BlockKey {
    const fn new(order: Order, start: PhysicalAddress) -> BlockKey {
        BlockKey { order, start }
    }

    #[inline]
    fn index(&self) -> usize {
        self.start.as_usize() / self.order.bytes()
    }

    #[inline]
    fn buddy(&self) -> BlockKey {
        if self.index() % 2 == 0 {
            BlockKey::new(self.order, self.start + self.order.bytes())
        } else {
            BlockKey::new(self.order, self.start - self.order.bytes())
        }
    }

    #[inline]
    fn parent(&self) -> Option<BlockKey> {
        self.order.parent().map(|p| BlockKey::new(p, self.start))
    }

    #[inline]
    fn left_child(&self) -> Option<BlockKey> {
        self.order.child().map(|c| BlockKey::new(c, self.start))
    }

    #[inline]
    fn right_child(&self) -> Option<BlockKey> {
        self.order.child().map(|c| BlockKey::new(c, self.start + c.bytes()))
    }
}

pub struct PhysicalAllocator {
    free_lists: [RBTree<FreeBlockAdapter>; Order::COUNT],
}

impl PhysicalAllocator {
    const fn new() -> PhysicalAllocator {
        PhysicalAllocator {
            free_lists: arr![RBTree::new(FreeBlockAdapter::NEW); 12]
        }
    }

    fn free_block_cursor_mut(&mut self, key: BlockKey) -> CursorMut<FreeBlockAdapter> {
        self.free_lists[key.order.as_usize()].find_mut(&key.start)
    }

    fn insert_free_block(&mut self, key: BlockKey) {
        let block_addr = physical_to_virtual(key.start);
        let block: &'static mut FreeBlock = unsafe { block_addr.as_mut_ref() };
        block.start = key.start;
        block.order = key.order;
        block.link = RBTreeLink::new();

        self.free_lists[key.order.as_usize()].insert(block);
    }

    /// Free the block of physical memory represented by the given `BlockKey`
    fn return_block(&mut self, key: BlockKey) {
        if let Some(parent) = key.parent() {
            let mut buddy = self.free_block_cursor_mut(key.buddy());

            if buddy.is_null() {
                trace!("Returning {:?}", key);
                self.insert_free_block(key);
            } else {
                let buddy_block = buddy.get().unwrap();
                assert_eq!(buddy_block.order, key.order);
                trace!("Reuniting {:?} with {:?}", key, buddy_block);
                buddy.remove();
                self.return_block(parent); // Recurse up in case there's more to combine
            }
        } else {
            trace!("Returning {:?}", key);
            self.insert_free_block(key);
        }
    }

    /// Find a free block of the requested order and remove it from the free list. This method will
    /// split higher-order blocks if necessary
    fn take_block(&mut self, order: Order) -> Option<BlockKey> {
        let mut head = self.free_lists[order.as_usize()].front_mut();

        match head.remove() {
            Some(block) => {
                assert_eq!(block.order, order);
                let key = BlockKey::new(block.order, block.start);
                trace!("Taking {:?}", key);
                Some(key)
            },
            None => {
                if let Some(parent) = order.parent().and_then(|o| self.take_block(o)) {
                    assert_eq!(Some(parent.order), order.parent());
                    trace!("Splitting {:?}", parent);

                    self.insert_free_block(parent.right_child().unwrap());
                    // unwrap and re-wrap to force a panic if something weird happened
                    Some(parent.left_child().unwrap())
                } else {
                    None
                }
            }
        }
    }

    pub fn allocate(&mut self, frames: usize) -> Option<PhysicalAddress> {
        if frames == 0 {
            return None;
        }

        trace!("Allocating {} frames", frames);
        Order::for_frames(frames)
            .and_then(|order| self.take_block(order))
            .map(|k| k.start)

        // TODO: mark waste in block as free
    }

    pub fn free(&mut self, frames: usize, start: PhysicalAddress) {
        trace!("Freeing {} frames starting at {}", frames, start);
        let order = Order::for_frames(frames).expect("Attempted to free allocation of invalid size");

        self.return_block(BlockKey::new(order, start));
    }

    /// Add a contiguous range of memory to the allocator
    ///
    /// # Arguments
    /// - `start`: the start of the range (inclusive)
    /// - `end`: the end of the range (exclusive)
    pub fn add_range(&mut self, start: PhysicalAddress, end: PhysicalAddress) {
        assert!(start.is_aligned(FRAME_SIZE), "Start address is not page-aligned");
        assert!(end.is_aligned(FRAME_SIZE), "End address is not page-aligned");

        let mut start = start;
        let mut order = Order::MAX;
        loop {
            while (end - start).as_usize() >= order.bytes() {
                let key = BlockKey::new(order, start);
                trace!("Adding {:?} to allocator", key);
                self.insert_free_block(key);
                start += order.bytes()
            }

            if let Some(next) = order.child() {
                order = next;
            } else {
                break
            }
        }
    }
}

// intrusive_collections::rbtree::Link uses Cell, so it's not Sync. PhysicalAllocator should be
// Sync because all of the public APIs which allow mutation take &mut self
unsafe impl Sync for PhysicalAllocator {}
unsafe impl Send for PhysicalAllocator {}

pub static PHYSICAL_ALLOCATOR: Mutex<PhysicalAllocator> = Mutex::new(PhysicalAllocator::new());

// Convenience functions for callers, so they don't have to deal with the lock

/// Allocate `frames` frames of physical memory
///
/// Returns `None` if insufficient memory is available
pub fn allocate_frames(frames: usize) -> Option<PhysicalAddress> {
    PHYSICAL_ALLOCATOR.lock().allocate(frames)
}

/// Free `frames` frames of physical memory starting at `start`
pub fn free_frames(frames: usize, start: PhysicalAddress) {
    PHYSICAL_ALLOCATOR.lock().free(frames, start)
}