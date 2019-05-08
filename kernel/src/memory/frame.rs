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

use core::mem;
use core::ptr;
use core::slice;

use bit_field::BitArray;
use bootloader::BootInfo;
use bootloader::bootinfo::{MemoryRegionType, FrameRange};
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use log::{info, trace};
use spin::Mutex;
use x86_64::VirtAddr;

const MAX_ORDER: usize = 11;
const FRAME_SIZE: usize = 4096;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct BlockId {
    order: usize,
    index: usize,
}

impl BlockId {
    // Unfortunately, can't use assertions in const fns
    fn new(order: usize, index: usize) -> BlockId {
        debug_assert!(order <= MAX_ORDER);
        // TODO: check index is valid
        BlockId { order, index }
    }

    #[inline(always)]
    fn order(&self) -> usize {
        self.order
    }

    #[inline(always)]
    fn index(&self) -> usize {
        self.index
    }

    #[inline(always)]
    fn sibling(&self) -> BlockId {
        BlockId::new(self.order, if self.index % 2 == 0 { self.index + 1} else { self.index - 1 })
    }

    #[inline(always)]
    fn parent(&self) -> Option<BlockId> {
        if self.order < MAX_ORDER {
            let parent = (self.index & !1) >> 1;
            Some(BlockId::new(self.order + 1, parent))
        } else {
            None
        }
    }

    #[inline(always)]
    fn left_child(&self) -> BlockId {
        debug_assert!(self.order > 0);
        BlockId::new(self.order - 1, self.index << 1)
    }

    #[inline(always)]
    fn right_child(&self) -> BlockId {
        debug_assert!(self.order > 0);
        BlockId::new(self.order - 1, (self.index << 1) + 1)
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
    bitmaps: [&'static mut [u8]; MAX_ORDER + 1],

    // Free lists for each order
    free_lists: [LinkedList<FreeBlockAdapter>; MAX_ORDER + 1],
}

impl RegionInner {
    // Size of the region header (Region struct and bitmaps)
    const HEADER_SIZE: usize = 2 * FRAME_SIZE;

    fn mark_allocated(&mut self, block: BlockId, allocated: bool) {
        self.bitmaps[block.order()].set_bit(block.index(), allocated)
    }

    fn is_allocated(&self, block: BlockId) -> bool {
        self.bitmaps[block.order()].get_bit(block.index())
    }

    // NOTE: to ensure the header is _never_ allocated and make initialization a bit easier, it's
    // not part of the allocatable region. That's why block_address and block_id add/subtract HEADER_SIZE

    fn block_address(&self, block: BlockId) -> VirtAddr {
        self.region_start + block.index() * order_frames(block.order()) * FRAME_SIZE + RegionInner::HEADER_SIZE
    }

    fn block_id(&self, addr: VirtAddr, order: usize) -> BlockId {
        debug_assert!(addr < self.region_start + self.num_frames as usize * FRAME_SIZE, "Block does not belong to region");

        let frame_offset = ((addr - self.region_start - RegionInner::HEADER_SIZE as u64) / FRAME_SIZE as u64) as usize;
        let index = frame_offset / order_frames(order);
        BlockId::new(order, index)
    }

    fn free(&mut self, block: BlockId) {
        debug_assert!(self.is_allocated(block), "Freeing a block that isn't allocated");

        if let Some(parent) = block.parent() {
            // Not at the top, so we can try merging with our sibling
            if self.is_allocated(block.sibling()) {
                let free_block = unsafe { FreeBlock::create_at(self.block_address(block), block.order()) };
                self.free_lists[block.order()].push_front(free_block);
                self.mark_allocated(block, false);
                trace!("Freed {:?}", block);
            } else {
                assert!(self.is_allocated(parent), "Parent of allocated block must be allocated");

                // Need to un-free sibling for merging
                self.mark_allocated(block.sibling(), true);
                let sibling = unsafe { FreeBlock::from_address(self.block_address(block.sibling())) };
                debug_assert!(sibling.order == block.order(), "Sibling has wrong order");
                debug_assert!(sibling.link.is_linked(), "Sibling should be in the free list");
                unsafe { self.free_lists[block.order()].cursor_mut_from_ptr(sibling) }.remove();

                self.free(parent);
            }
        } else {
            let free_block = unsafe { FreeBlock::create_at(self.block_address(block), block.order()) };
            self.free_lists[block.order()].push_front(free_block);
            self.mark_allocated(block, false);
            trace!("Freed {:?}", block);
        }
    }

    fn alloc(&mut self, order: usize) -> Option<BlockId> {
        debug_assert!(order <= MAX_ORDER);

        if let Some(block) = self.free_lists[order].pop_front() {
            debug_assert!(block.order == order);
            let block_id = self.block_id(VirtAddr::from_ptr(block), order);
            self.mark_allocated(block_id, true);
            trace!("Allocating {:?}", block_id);
            Some(block_id)
        } else if order < MAX_ORDER {
            if let Some(parent) = self.alloc(order + 1) {
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
}

impl Region {
    const MAX_FRAMES: u64 = 8 * order_frames(MAX_ORDER) as u64;

    fn new(start: VirtAddr, num_frames: u64) -> &'static Region {
        assert!(num_frames > 2, "Region of size {} is not large enough", num_frames);
        assert!(num_frames <= Region::MAX_FRAMES, "Region cannot support {} frames", num_frames);

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

        for order in 0..=MAX_ORDER {
            let bitmap_size = 1 << (MAX_ORDER - order);
            let bitmap_addr: VirtAddr = bitmaps_start + (bitmap_size - 1usize);

            inner.bitmaps[order] = unsafe {
                let start_ptr = bitmap_addr.as_mut_ptr();
                ptr::write_bytes(start_ptr, 0xff, bitmap_size as usize); // Mark everything as allocated
                slice::from_raw_parts_mut(start_ptr, bitmap_size as usize)
            };

            inner.free_lists[order] = LinkedList::new(FreeBlockAdapter::new());

            info!("Order-{} bitmap is {} bytes long", order, inner.bitmaps[order].len());
        }

        let avail_frames = num_frames - 2; // Header is 2 pages
        let mut frame_start = 0;
        let mut order = MAX_ORDER;

        while frame_start < num_frames {
            let remaining = num_frames - frame_start;
            let nframes = order_frames(order);
            if nframes <= remaining as usize {
                info!("Marking order-{} block starting at offset {} as free", order, frame_start);
                inner.bitmaps[order].set_bit(frame_start as usize >> order, false);

                let block = unsafe { FreeBlock::create_at(start + ((frame_start + 2) * FRAME_SIZE as u64), order) };
                inner.free_lists[order].push_back(block);

                frame_start += nframes as u64;
            } else {
                order -= 1;
            }
        }

        region
    }

    fn alloc(&self, order: usize) -> Option<*mut u8> {
        let mut inner = self.inner.lock();
        inner.alloc(order)
            .map(|id| inner.block_address(id).as_mut_ptr())
        // TODO: memory poisoning would be nice if there's a fast enough way to fill entire pages
    }

    fn free(&self, order: usize, block: *mut u8) {
        let mut inner = self.inner.lock();

        let id = inner.block_id(VirtAddr::from_ptr(block), order);
        inner.free(id);
    }
}

struct FreeBlock {
    link: LinkedListLink,
    order: usize, // for debugging
}

impl FreeBlock {
    unsafe fn from_address(addr: VirtAddr) -> &'static FreeBlock {
        let ptr: *mut FreeBlock = addr.as_mut_ptr();
        ptr.as_mut().unwrap()
    }

    unsafe fn create_at(addr: VirtAddr, order: usize) -> &'static FreeBlock {
        let ptr: *mut FreeBlock = addr.as_mut_ptr();
        let block = ptr.as_mut().unwrap();
        block.link = LinkedListLink::new();
        block.order = order;
        block
    }
}

intrusive_adapter!(FreeBlockAdapter = &'static FreeBlock : FreeBlock { link: LinkedListLink });

/// Given an order, returns the number of page frames in a block of that order
const fn order_frames(order: usize) -> usize {
    1 << order
}

intrusive_adapter!(RegionAdapter = &'static Region : Region { link: LinkedListLink });

pub struct FrameAllocator {
    regions: LinkedList<RegionAdapter>,
}

impl FrameAllocator {
    const fn new() -> FrameAllocator {
        FrameAllocator {
            regions: LinkedList::new(RegionAdapter::new())
        }
    }

    fn add_range(&mut self, frame_range: FrameRange, map_offset: u64) {
        let mut start_frame = frame_range.start_frame_number;

        // We might need multiple Regions to span the range
        while start_frame + Region::MAX_FRAMES <= frame_range.end_frame_number {
            let start_addr = VirtAddr::new(start_frame * (FRAME_SIZE as u64) + map_offset);
            self.regions.push_back(Region::new(start_addr, Region::MAX_FRAMES));
            start_frame += Region::MAX_FRAMES;
        }

        // Add a Region for any remaining frames less than the max
        if start_frame < frame_range.end_frame_number {
            let start_addr = VirtAddr::new(start_frame * (FRAME_SIZE as u64) + map_offset);
            let num_frames = frame_range.end_frame_number - start_frame;
            self.regions.push_back(Region::new(start_addr, num_frames));
        }
    }
}


pub fn init(boot: &BootInfo) -> FrameAllocator {
    assert!(mem::size_of::<Region>() <= FRAME_SIZE);

    let mut allocator = FrameAllocator::new();

    for region in boot.memory_map.iter() {
        if region.region_type == MemoryRegionType::Usable {
            allocator.add_range(region.range, boot.physical_memory_offset);
        }
    }

    for region in allocator.regions.iter() {
        let addr = VirtAddr::from_ptr(region as *const Region);
        info!("Region at {:?} ({:?})", addr, addr - boot.physical_memory_offset);

        {
            let inner = region.inner.lock();

            for order in 0..=MAX_ORDER {
                let bitmap_addr = VirtAddr::from_ptr(inner.bitmaps[order].as_ptr());
                info!("    Order {} bitmap at {:?}", order, bitmap_addr);
            }
        }
    }

    allocator
}

#[cfg(test)]
#[test_case]
fn test_block_id() {
    let b = BlockId::new(0, 0);
    assert_eq!(b.sibling(), BlockId::new(0, 1));
    assert_eq!(b.parent(), Some(BlockId::new(1, 0)));

    let b = BlockId::new(1, 2);
    assert_eq!(b.parent(), Some(BlockId::new(2, 1)));
    assert_eq!(b.left_child(), BlockId::new(0, 4));
    assert_eq!(b.right_child(), BlockId::new(0, 5));


}