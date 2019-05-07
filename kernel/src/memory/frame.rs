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
use log::info;
use spin::Mutex;
use x86_64::VirtAddr;

const MAX_ORDER: usize = 11;
const FRAME_SIZE: usize = 4096;

struct Region {
    /// Link in the parent FrameAllocator
    link: LinkedListLink,

    /// Number of 4KiB physical page frames
    num_frames: u64,

    /// Inner, mutable state
    inner: Mutex<RegionInner>,
}

/// Inner, mutable state for a region. This layering is necessary because intrusive collection
/// members need to be immutable (see https://github.com/Amanieu/intrusive-rs/issues/19). To get
/// around this, all the mutable bits go in an inner struct, which the outer one wraps in a Mutex.
/// Then, we can pass around &'static Region pointers and keep all the state for a region in its
/// first page.
struct RegionInner {
    // Bitmap tree
    bitmaps: [&'static mut [u8]; MAX_ORDER + 1],

    // Free lists for each order
    free_lists: [LinkedList<FreeBlockAdapter>; MAX_ORDER + 1],
}

// TODO: for consistency, make this just deal with indices internally, except where needed (free list)

impl RegionInner {
    fn mark_allocated<T>(&mut self, region: &Region, block: &T, order: usize, allocated: bool) {
        self.bitmaps[order].set_bit(region.block_index(block, order), allocated)
    }

    fn is_allocated<T>(&self, region: &Region, block: &T, order: usize) -> bool {
        self.bitmaps[order].get_bit(region.block_index(block, order))
    }

    fn free(&mut self, region: &Region, block: *mut u8, order: usize) {
        let index = region.block_index(unsafe { block.as_ref() }.unwrap(), order);
        assert!(self.bitmaps[order].get_bit(index), "Freeing an already-freed block");

        // From David's fancy bit manipulation
        let sibling = (index) - 1 + (((index) & 1) << 1);

        if self.bitmaps[order].get_bit(sibling) || order == MAX_ORDER {
            let block = unsafe { FreeBlock::from_address(VirtAddr::from_ptr(block)) };
            self.free_lists[order].push_back(block);
            self.mark_allocated(region, block, order, false);
        } else {
            // Reunite the buddies!

            let sibling_block: *const FreeBlock = region.block_addr::<FreeBlock>(sibling, order).as_ptr();
            unsafe { self.free_lists[order].cursor_mut_from_ptr(sibling_block) }.remove();
            self.bitmaps[order].set_bit(sibling, true);

            let parent_idx = (index - 1) >> 1;
            assert!(self.bitmaps[order + 1].get_bit(parent_idx), "Parent of allocated block should be allocated");

            self.free(region, region.block_addr::<u8>(parent_idx, order + 1).as_mut_ptr(), order + 1);
        }
    }

    fn alloc(&mut self, region: &Region, order: usize) -> Option<*mut u8> {
        if let Some(block) = self.free_lists[order].pop_front() {
            self.mark_allocated(region, block, order, true);

            unsafe { Some(mem::transmute(block)) }
        } else if order < MAX_ORDER {
            // Split a parent block

            let parent = self.alloc(region, order + 1);
            if let Some(parent) = parent {
                let buddy = unsafe { parent.offset((order_frames(order) * FRAME_SIZE) as isize) };
                self.free(region, buddy, order);
                self.mark_allocated(region, unsafe { parent.as_ref() }.unwrap(), order, true);
                Some(parent)
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

        region.num_frames = num_frames;

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

        let mut frame_start = 2;
        let mut order = MAX_ORDER;

        while frame_start < num_frames {
            let remaining = num_frames - frame_start;
            let nframes = order_frames(order);
            if nframes <= remaining as usize {
                info!("Marking order-{} block starting at offset {} as free", order, frame_start);
                inner.bitmaps[order].set_bit(frame_start as usize >> order, false);

                let block = unsafe { FreeBlock::from_address(start + (frame_start * FRAME_SIZE as u64)) };
                inner.free_lists[order].push_back(block);

                frame_start += nframes as u64;
            } else {
                order -= 1;
            }
        }

        region
    }

    fn block_index<T>(&self, block: &T, order: usize) -> usize {
        let region_addr = (self as *const Region) as usize;
        let block_addr = (block as *const T) as usize;

        assert!(block_addr < region_addr + self.num_frames as usize * FRAME_SIZE, "Block does not belong to region");

        let frame_offset = (block_addr - region_addr) / FRAME_SIZE;
        frame_offset / order_frames(order)
    }

    fn block_addr<T>(&self, index: usize, order: usize) -> VirtAddr {
        let region_addr = (self as *const Region) as usize;
        VirtAddr::new((region_addr + order_frames(order) * index) as u64)
    }

    fn alloc(&self, order: usize) -> Option<*mut u8> {
        let mut inner = self.inner.lock();
        inner.alloc(self, order)
    }

    fn free(&self, order: usize, block: *mut u8) {
        let mut inner = self.inner.lock();
        inner.free(self, block, order);
    }
}

struct FreeBlock {
    link: LinkedListLink
}

impl FreeBlock {
    unsafe fn from_address(addr: VirtAddr) -> &'static FreeBlock {
        let ptr: *mut FreeBlock = addr.as_mut_ptr();
        let block = ptr.as_mut().unwrap();
        block.link = LinkedListLink::new();
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
        info!("Region at {:?}", addr - boot.physical_memory_offset);

        {
            let inner = region.inner.lock();

            for order in 0..=MAX_ORDER {
                let bitmap_addr = VirtAddr::from_ptr(inner.bitmaps[order].as_ptr());
                info!("    Order {} bitmap at {:?}", order, bitmap_addr - boot.physical_memory_offset);
            }
        }

        for i in 0..20 {
            let test_block = region.alloc(2);
            info!("Allocated block {:?}", test_block);

            if i % 2 == 0 {
                if let Some(test_block) = test_block {
                    region.free(2, test_block);
                    info!("Freed block");
                }
            }
        }
    }

    allocator
}