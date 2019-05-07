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
use x86_64::{PhysAddr, VirtAddr};

use kutil::bitmap::Bitmap;

const MAX_ORDER: usize = 11;
const FRAME_SIZE: usize = 4096;

struct Region {
    /// Number of 4KiB physical page frames
    num_frames: u64,

    // Bitmap tree
    bitmaps: [&'static mut [u8]; MAX_ORDER + 1],

    // Free lists for each order
    free_lists: [LinkedList<FreeBlockAdapter>; MAX_ORDER + 1],

    // Link in the global region list
    link: LinkedListLink,
}

impl Region {
    const MAX_FRAMES: u64 = 8 * order_frames(MAX_ORDER) as u64;

    fn new(start: VirtAddr, num_frames: u64) -> &'static Region {
        assert!(num_frames > 2, "Region of size {} is not large enough", num_frames);
        assert!(num_frames <= Region::MAX_FRAMES, "Region cannot support {} frames", num_frames);

        let region: *mut Region = start.as_mut_ptr();
        let region: &'static mut Region = unsafe { region.as_mut().unwrap() };

        region.num_frames = num_frames;
        region.link = LinkedListLink::new();

        // Bitmaps start in the second page of the region
        let bitmaps_start = start + FRAME_SIZE;

        for order in 0..=MAX_ORDER {
            let bitmap_size = 1 << (MAX_ORDER - order);
            let bitmap_addr: VirtAddr = bitmaps_start + (bitmap_size - 1usize);

            region.bitmaps[order] = unsafe {
                let start_ptr = bitmap_addr.as_mut_ptr();
                ptr::write_bytes(start_ptr, 0xff, bitmap_size as usize); // Mark everything as allocated
                slice::from_raw_parts_mut(start_ptr, bitmap_size as usize)
            };

            region.free_lists[order] = LinkedList::new(FreeBlockAdapter::new());

            info!("Order-{} bitmap is {} bytes long", order, region.bitmaps[order].len());
        }

        let mut frame_start = 2;
        let mut order = MAX_ORDER;

        while frame_start < num_frames {
            let remaining = num_frames - frame_start;
            let nframes = order_frames(order);
            if nframes <= remaining as usize {
                info!("Marking order-{} block starting at offset {} as free", order, frame_start);
                region.bitmaps[order].set_bit(frame_start as usize >> order, false);

                let block = unsafe { FreeBlock::from_address(start + (frame_start * FRAME_SIZE as u64)) };
                region.free_lists[order].push_back(block);

                frame_start += nframes as u64;
            } else {
                order -= 1;
            }
        }

        region
    }
}

intrusive_adapter!(RegionAdapter = &'static Region : Region { link: LinkedListLink });

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

        for order in 0..=MAX_ORDER {
            let bitmap_addr = VirtAddr::from_ptr(region.bitmaps[order].as_ptr());
            info!("    Order {} bitmap at {:?}", order, bitmap_addr - boot.physical_memory_offset);
        }
    }

    allocator
}