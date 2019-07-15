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
#![no_std]

mod block_id;
mod order;
mod region;

use core::mem;

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use log::info;

use order::Order;
use region::{Region, RegionAdapter};

#[cfg(target_arch = "x86_64")]
const FRAME_SIZE: usize = 4096;

// TODO: Box-like pointer type for page frame allocations that frees on drop

pub struct FrameAllocator {
    /// Start address of the kernel's physical memory map
    physical_memory_map: usize,
    regions: LinkedList<RegionAdapter>
}

impl FrameAllocator {
    /// Create a new `FrameAllocator`. This allocator will not have any physical memory regions, so
    /// all allocations will fail until some are added with `add_range`.
    ///
    /// # Arguments
    /// * `physical_memory_map` - the starting virtual address of the kernel's physical memory map
    pub fn new(physical_memory_map: usize) -> FrameAllocator {
        debug_assert!(mem::size_of::<Region>() <= FRAME_SIZE);

        FrameAllocator {
            physical_memory_map,
            regions: LinkedList::new(RegionAdapter::new())
        }
    }

    /// Add a range of physical memory for this allocator to use. Note that the frame numbers
    /// used here are _physical_, not virtual. Also note that `end_frame` is exclusive.
    pub fn add_range(&mut self, start_frame: usize, end_frame: usize) {
        let mut start_frame = start_frame;
        // We might need multiple Regions to span the range
        while start_frame + Region::MAX_FRAMES <= end_frame {
            let start_addr = start_frame * FRAME_SIZE + self.physical_memory_map;
            info!(
                "Adding {}-frame region starting at {:#x}",
                Region::MAX_FRAMES,
                start_addr
            );
            self.regions
                .push_back(Region::new(start_addr, Region::MAX_FRAMES));
            start_frame += Region::MAX_FRAMES;
        }

        // Add a Region for any remaining frames less than the max
        if start_frame < end_frame {
            let start_addr = start_frame * FRAME_SIZE + self.physical_memory_map;
            let num_frames = end_frame - start_frame;
            info!(
                "Adding {}-frame region starting at {:#x}",
                num_frames, start_addr
            );
            self.regions.push_back(Region::new(start_addr, num_frames));
        }
    }

    pub fn allocate_pages(&self, npages: usize) -> Option<FrameAllocation> {
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

                let phys_start = PhysFrame::from_start_address(PhysAddr::new(
                    allocation.as_u64() - self.physical_memory_offset,
                ))
                    .expect("Allocation was not page-aligned");
                let virt_start =
                    Page::from_start_address(allocation).expect("Allocation was not page-aligned");

                return Some(FrameAllocation {
                    start: phys_start,
                    mapped_start: virt_start,
                    npages,
                });
            }
        }

        None
    }
}