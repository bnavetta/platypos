//!
//! Physical page allocator using buddy bitmaps.
//!
//! Physical memory is split into contiguous ranges called `Region`s. Each region has a bitmap tree
//! of a fixed depth of 12, with 8 entries at the top level (which ensures that the entire tree fits
//! in a page). If the bitmap tree describes more memory than the region actually contains, the
//! excess is permanently marked as allocated. Alternatively, there can be multiple directly adjacent
//! regions if there's a chunk of memory too big for a single region to represent.
//!
//! There are a couple useful properties for figuring out how the bitmap is laid out. Consider an
//! order _k_:
//! * The bitmap takes up `2^(MAX_ORDER - k)` bytes
//! * The bitmap holds `2^(MAX_ORDER - k + 3)` entries
//! * The bitmap starts `2^(MAX_ORDER - k) - 1` bytes from the start of the tree
//!

use core::{cmp::min, mem, ptr, slice};

use bit_field::BitArray;
use bootloader::{
    bootinfo::{FrameRange, MemoryRegionType},
    BootInfo,
};
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use kutil::log2;
use log::{info, trace};
use spin::Mutex;
use x86_64::{
    instructions::interrupts,
    structures::paging::{self, PhysFrame},
    PhysAddr, VirtAddr,
};

use super::FRAME_SIZE;

#[cfg(test)]
use crate::tests;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
struct Order(u8);

impl Order {
    const MAX: Order = Order(11);
    const MAX_VAL: usize = 11;

    const MIN: Order = Order(0);

    /// Returns the number of frames in a block of this order
    const fn frames(&self) -> usize {
        1usize << self.0
    }

    /// Returns the number of bytes in a block of this order
    const fn bytes(&self) -> usize {
        self.frames() * FRAME_SIZE
    }

    /// Returns the maximum allowed index for a block of this order. This relies on assuming a
    /// one-page tree, where the order-11 bitmap occupies 1 byte
    const fn max_index(&self) -> usize {
        // See the properties for orders listed above
        1 << (Order::MAX_VAL - self.as_usize() + 3)
    }

    fn parent(&self) -> Order {
        debug_assert!(*self < Order::MAX);
        Order(self.0 + 1)
    }

    fn child(&self) -> Order {
        debug_assert!(self.0 > 0);
        Order(self.0 - 1)
    }

    const fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl From<u8> for Order {
    fn from(v: u8) -> Order {
        debug_assert!((v as usize) <= Order::MAX_VAL);
        Order(v)
    }
}

impl From<usize> for Order {
    fn from(v: usize) -> Order {
        debug_assert!(v <= Order::MAX_VAL);
        Order(v as u8)
    }
}

impl Into<usize> for Order {
    fn into(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct BlockId {
    order: Order,
    index: usize,
}

impl BlockId {
    // Unfortunately, can't use assertions in const fns
    fn new(order: Order, index: usize) -> BlockId {
        debug_assert!(index <= order.max_index());
        BlockId { order, index }
    }

    #[inline(always)]
    fn order(&self) -> Order {
        self.order
    }

    #[inline(always)]
    fn index(&self) -> usize {
        self.index
    }

    #[inline(always)]
    fn sibling(&self) -> BlockId {
        BlockId::new(
            self.order,
            if self.index % 2 == 0 {
                self.index + 1
            } else {
                self.index - 1
            },
        )
    }

    #[inline(always)]
    fn parent(&self) -> Option<BlockId> {
        if self.order < Order::MAX {
            let parent = (self.index & !1) >> 1;
            Some(BlockId::new(self.order.parent(), parent))
        } else {
            None
        }
    }

    #[inline(always)]
    fn left_child(&self) -> BlockId {
        debug_assert!(self.order > Order::MIN);
        BlockId::new(self.order.child(), self.index << 1)
    }

    #[inline(always)]
    fn right_child(&self) -> BlockId {
        debug_assert!(self.order > Order::MIN);
        BlockId::new(self.order.child(), (self.index << 1) + 1)
    }
}

struct Region {
    /// Link in the parent FrameAllocator
    link: LinkedListLink,

    /// Inner, mutable state
    inner: Mutex<RegionInner>,
}

/// Inner, mutable state for a region. This layering is necessary because intrusive collection
/// members need to be immutable (see https://github.com/Amanieu/intrusive-rs/issues/19). To get
/// around this, all the mutable bits go in an inner struct, which the outer one wraps in a Mutex.
/// Then, we can pass around &'static Region pointers and keep all the state for a region in its
/// first page.
struct RegionInner {
    /// Number of 4KiB physical page frames
    num_frames: u64,

    /// We hold on to this here for convenience - it's nicer than doing sketchy pointer arithmetic
    region_start: VirtAddr,

    // Bitmap tree
    bitmaps: [&'static mut [u8]; Order::MAX_VAL + 1],

    // Free lists for each order
    free_lists: [LinkedList<FreeBlockAdapter>; Order::MAX_VAL + 1],
}

impl RegionInner {
    // Size of the region header (Region struct and bitmaps)
    const HEADER_SIZE: usize = 2 * FRAME_SIZE;

    #[inline]
    fn start_addr(&self) -> VirtAddr {
        self.region_start
    }

    #[inline]
    fn end_addr(&self) -> VirtAddr {
        self.region_start + (self.num_frames * FRAME_SIZE as u64)
    }

    // NOTE: to ensure the header is _never_ allocated and make initialization a bit easier, it's
    // not part of the allocatable region. Thus, the start of the data portion of the region starts
    // 2 pages after the region start.

    #[inline]
    fn data_start_addr(&self) -> VirtAddr {
        self.region_start + RegionInner::HEADER_SIZE
    }

    #[inline]
    fn data_frames(&self) -> usize {
        self.num_frames as usize - 2
    }

    #[inline]
    fn mark_allocated(&mut self, block: BlockId, allocated: bool) {
        self.bitmaps[block.order().as_usize()].set_bit(block.index(), allocated)
    }

    #[inline]
    fn is_allocated(&self, block: BlockId) -> bool {
        self.bitmaps[block.order().as_usize()].get_bit(block.index())
    }

    fn block_address(&self, block: BlockId) -> VirtAddr {
        debug_assert!(
            block.order() <= self.max_order(),
            "Block does not fit in region"
        );
        debug_assert!(
            block.index() <= self.max_index(block.order()),
            "Block does not fit in region"
        );
        self.data_start_addr() + block.index() * block.order().bytes()
    }

    fn block_id(&self, addr: VirtAddr, order: Order) -> BlockId {
        debug_assert!(
            self.contains(addr),
            "Block {:?} does not belong to region",
            addr
        );
        debug_assert!(
            addr.is_aligned(FRAME_SIZE as u64),
            "Address must be page-aligned"
        );

        let index = (addr - self.data_start_addr()) as usize / FRAME_SIZE / order.frames();
        BlockId::new(order, index)
    }

    /// Returns the highest index within this region for a block of the given order
    fn max_index(&self, order: Order) -> usize {
        // subtract 1 since 0-indexed
        self.data_frames() / order.frames() - 1
    }

    /// Returns the largest order which can be allocated in this region
    fn max_order(&self) -> Order {
        Order::from(min(log2(self.data_frames()), Order::MAX_VAL))
    }

    fn free(&mut self, block: BlockId) {
        debug_assert!(
            self.is_allocated(block),
            "Freeing a block that isn't allocated"
        );

        if let Some(parent) = block.parent() {
            // Not at the top, so we can try merging with our sibling
            if self.is_allocated(block.sibling()) {
                let free_block =
                    unsafe { FreeBlock::create_at(self.block_address(block), block.order()) };
                self.free_lists[block.order().as_usize()].push_front(free_block);
                self.mark_allocated(block, false);
                trace!("Freed {:?}", block);
            } else {
                assert!(
                    self.is_allocated(parent),
                    "Parent of allocated block must be allocated"
                );

                // Need to un-free sibling for merging
                self.mark_allocated(block.sibling(), true);
                let sibling =
                    unsafe { FreeBlock::from_address(self.block_address(block.sibling())) };
                debug_assert!(sibling.order == block.order(), "Sibling has wrong order");
                debug_assert!(
                    sibling.link.is_linked(),
                    "Sibling should be in the free list"
                );
                unsafe { self.free_lists[block.order().as_usize()].cursor_mut_from_ptr(sibling) }
                    .remove();

                self.free(parent);
            }
        } else {
            let free_block =
                unsafe { FreeBlock::create_at(self.block_address(block), block.order()) };
            self.free_lists[block.order().as_usize()].push_front(free_block);
            self.mark_allocated(block, false);
            trace!("Freed {:?}", block);
        }
    }

    fn alloc(&mut self, order: Order) -> Option<BlockId> {
        if let Some(block) = self.free_lists[order.as_usize()].pop_front() {
            debug_assert!(block.order == order);
            let block_id = self.block_id(VirtAddr::from_ptr(block), order);
            self.mark_allocated(block_id, true);
            trace!("Allocating {:?}", block_id);
            Some(block_id)
        } else if order < Order::MAX {
            if let Some(parent) = self.alloc(order.parent()) {
                let block = parent.left_child();
                let sibling = parent.right_child();

                self.mark_allocated(block, true);
                self.free(sibling);
                trace!("Allocating {:?}", block);
                Some(block)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn contains(&self, addr: VirtAddr) -> bool {
        addr >= self.start_addr() && addr < self.end_addr()
    }
}

impl Region {
    const MAX_FRAMES: u64 = 8 * Order::MAX.frames() as u64;

    fn new(start: VirtAddr, num_frames: u64) -> &'static Region {
        assert!(
            num_frames > 2,
            "Region of size {} is not large enough",
            num_frames
        );
        assert!(
            num_frames <= Region::MAX_FRAMES,
            "Region cannot support {} frames",
            num_frames
        );

        let region: *mut Region = start.as_mut_ptr();
        let region: &'static mut Region = unsafe { region.as_mut().unwrap() };
        region.link = LinkedListLink::new();
        // TODO: Using mem::zeroed() is kinda hacky, probably better to fully initialize RegionInner before setting on the Region
        region.inner = Mutex::new(unsafe { mem::zeroed() });

        let mut inner = region.inner.lock();

        inner.num_frames = num_frames;
        inner.region_start = start;

        // Bitmaps start in the second page of the region
        let bitmaps_start = start + FRAME_SIZE;

        for order in 0..=Order::MAX_VAL {
            let bitmap_size = 1 << (Order::MAX_VAL - order);
            let bitmap_addr: VirtAddr = bitmaps_start + (bitmap_size - 1usize);

            inner.bitmaps[order] = unsafe {
                let start_ptr = bitmap_addr.as_mut_ptr();
                ptr::write_bytes(start_ptr, 0xff, bitmap_size as usize); // Mark everything as allocated
                slice::from_raw_parts_mut(start_ptr, bitmap_size as usize)
            };

            inner.free_lists[order] = LinkedList::new(FreeBlockAdapter::new());
        }

        let avail_frames = num_frames - 2; // Header is 2 pages
        let mut frame_start = 0;
        let mut order = Order::MAX;

        while frame_start < avail_frames {
            let remaining = avail_frames - frame_start;
            if order.frames() <= remaining as usize {
                inner.bitmaps[order.as_usize()]
                    .set_bit(frame_start as usize >> order.as_usize(), false);

                let block = unsafe {
                    FreeBlock::create_at(start + ((frame_start + 2) * FRAME_SIZE as u64), order)
                };
                inner.free_lists[order.as_usize()].push_back(block);

                frame_start += order.frames() as u64;
            } else {
                order = order.child();
            }
        }

        region
    }

    fn alloc(&self, order: usize) -> Option<*mut u8> {
        interrupts::without_interrupts(|| {
            let mut inner = self.inner.lock();
            inner
                .alloc(order.into())
                .map(|id| inner.block_address(id).as_mut_ptr())
            // TODO: memory poisoning would be nice if there's a fast enough way to fill entire pages
        })
    }

    fn free(&self, order: usize, block: *mut u8) {
        interrupts::without_interrupts(|| {
            let mut inner = self.inner.lock();

            let id = inner.block_id(VirtAddr::from_ptr(block), order.into());
            inner.free(id);
        });
    }

    fn contains(&self, addr: *const u8) -> bool {
        interrupts::without_interrupts(|| {
            let inner = self.inner.lock();
            inner.contains(VirtAddr::from_ptr(addr))
        })
    }
}

// Region otherwise wouldn't be Sync because LinkedListLink isn't Sync. This honestly seems like an
// issue with intrusive_collections - LinkedList is supposed to be Sync if the value type is Sync,
// but since LinkedListLink uses Cell, it seems like the value type never _can_ be Sync. Even using
// Arc doesn't seem like it'd work, since the constraint is on the value type. It should be fine
// here - the linked list of Regions is only modified when initializing the system, and the mutable
// parts (RegionInner) are wrapped in a Mutex anyways.
unsafe impl Sync for Region {}

struct FreeBlock {
    link: LinkedListLink,
    order: Order, // for debugging
}

impl FreeBlock {
    unsafe fn from_address(addr: VirtAddr) -> &'static FreeBlock {
        let ptr: *mut FreeBlock = addr.as_mut_ptr();
        ptr.as_mut().unwrap()
    }

    unsafe fn create_at(addr: VirtAddr, order: Order) -> &'static FreeBlock {
        let ptr: *mut FreeBlock = addr.as_mut_ptr();
        let block = ptr.as_mut().unwrap();
        block.link = LinkedListLink::new();
        block.order = order;
        block
    }
}

intrusive_adapter!(FreeBlockAdapter = &'static FreeBlock : FreeBlock { link: LinkedListLink });

intrusive_adapter!(RegionAdapter = &'static Region : Region { link: LinkedListLink });

pub struct FrameAllocator {
    physical_memory_offset: u64,
    regions: LinkedList<RegionAdapter>,
}

impl FrameAllocator {
    // unsafe because this must only be called once
    pub unsafe fn initialize(boot_info: &BootInfo) -> FrameAllocator {
        debug_assert!(mem::size_of::<Region>() <= FRAME_SIZE);

        info!("Initializing frame allocator");

        let mut allocator = FrameAllocator::new(boot_info.physical_memory_offset);

        for region in boot_info.memory_map.iter() {
            if region.region_type == MemoryRegionType::Usable {
                allocator.add_range(region.range);
            }
        }

        allocator
    }

    const fn new(physical_memory_offset: u64) -> FrameAllocator {
        FrameAllocator {
            regions: LinkedList::new(RegionAdapter::new()),
            physical_memory_offset,
        }
    }

    fn add_range(&mut self, frame_range: FrameRange) {
        let mut start_frame = frame_range.start_frame_number;

        // We might need multiple Regions to span the range
        while start_frame + Region::MAX_FRAMES <= frame_range.end_frame_number {
            let start_addr =
                VirtAddr::new(start_frame * (FRAME_SIZE as u64) + self.physical_memory_offset);
            info!(
                "Adding {}-frame region starting at {:?}",
                Region::MAX_FRAMES,
                start_addr
            );
            self.regions
                .push_back(Region::new(start_addr, Region::MAX_FRAMES));
            start_frame += Region::MAX_FRAMES;
        }

        // Add a Region for any remaining frames less than the max
        if start_frame < frame_range.end_frame_number {
            let start_addr =
                VirtAddr::new(start_frame * (FRAME_SIZE as u64) + self.physical_memory_offset);
            let num_frames = frame_range.end_frame_number - start_frame;
            info!(
                "Adding {}-frame region starting at {:?}",
                num_frames, start_addr
            );
            self.regions.push_back(Region::new(start_addr, num_frames));
        }
    }

    pub fn allocate_pages(&self, npages: usize) -> Option<*mut u8> {
        debug_assert!(npages > 0, "Must allocate at least one page");
        let order = log2(npages.next_power_of_two());
        debug_assert!(
            order < Order::MAX_VAL,
            "Cannot allocate {} pages at once",
            npages
        );

        for region in self.regions.iter() {
            if let Some(allocation) = region.alloc(order) {
                // TODO: to avoid wasting pages, there should be some way of marking the leftovers as free
                return Some(allocation);
            }
        }

        None
    }

    pub fn free_pages(&self, npages: usize, allocation: *mut u8) {
        debug_assert!(npages > 0, "Must free at least one page");
        let order = log2(npages.next_power_of_two());
        debug_assert!(
            order < Order::MAX_VAL,
            "Cannot free {} pages at once",
            npages
        );

        for region in self.regions.iter() {
            if region.contains(allocation) {
                region.free(order, allocation);
                return;
            }
        }

        panic!("No region contained {:#?}", allocation);
    }

    pub fn page_table_allocator(&self) -> PageTableAllocator {
        PageTableAllocator::new(self)
    }
}

/// Wrapper for allocating frames for page tables
pub struct PageTableAllocator<'a>(&'a FrameAllocator);

impl<'a> PageTableAllocator<'a> {
    pub fn new(allocator: &'a FrameAllocator) -> PageTableAllocator<'a> {
        PageTableAllocator(allocator)
    }
}

unsafe impl<'a, S: paging::PageSize> paging::FrameAllocator<S> for PageTableAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<paging::PhysFrame<S>> {
        self.0
            .allocate_pages(S::SIZE as usize / FRAME_SIZE)
            .map(|ptr| {
                let virt_addr = ptr as usize;
                let addr = PhysAddr::new(virt_addr as u64 - self.0.physical_memory_offset);
                paging::PhysFrame::from_start_address(addr)
                    .expect("Allocator returned a non-page-aligned address")
            })
    }
}

impl<'a, S: paging::PageSize> paging::FrameDeallocator<S> for PageTableAllocator<'a> {
    fn deallocate_frame(&mut self, frame: PhysFrame<S>) {
        let virt_addr =
            VirtAddr::new(frame.start_address().as_u64() + self.0.physical_memory_offset);
        self.0
            .free_pages(S::SIZE as usize / FRAME_SIZE, virt_addr.as_mut_ptr());
    }
}

#[cfg(test)]
tests! {
    test block_tree {
        let b = BlockId::new(Order(0), 0);
        assert_eq!(b.sibling(), BlockId::new(Order(0), 1));
        assert_eq!(b.parent(), Some(BlockId::new(Order(1), 0)));

        let b = BlockId::new(Order(1), 2);
        assert_eq!(b.parent(), Some(BlockId::new(Order(2), 1)));
        assert_eq!(b.left_child(), BlockId::new(Order(0), 4));
        assert_eq!(b.right_child(), BlockId::new(Order(0), 5));
    }

    test block_location {
        for region in allocator().regions.iter() {
            let inner = region.inner.lock();

            for order in 0..=inner.max_order().as_usize() {
                let order = Order::from(order);

                for index in 0..=inner.max_index(order) {
                    let block_id = BlockId::new(order, index);

                    let addr = inner.block_address(block_id);
                    assert_eq!(inner.block_id(addr, order), block_id);
                }
            }
        }
    }
}
