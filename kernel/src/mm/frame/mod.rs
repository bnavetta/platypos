//! Page frame allocator
//!
//! The allocator uses a combination of hierarchical (buddy) bitmaps and free lists. All allocations are in terms of orders, which define how many
//! frames are returned. To support sparsity (and eventually, NUMA and DMA requirements), the allocator divides the physical address space into regions,
//! each of which represents a contiguous region of usable physical memory. Each region tracks allocation state independently.
//!
//! Architecture-specific initialization code is responsible for creating a new `FrameAllocator` and calling `add_region` for each region of usable
//! physical memory.

use core::ops::Range;
use core::mem;

use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use slog::{info, debug};

use crate::KernelLogger;
use crate::mm::address::PageFrame;

mod bitmap;

/// An order is an allocatable block size. Blocks of order `n` contain `2^n` frames of physical memory.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct Order(u8);

impl Order {
    const MIN: Order = Order(0);
    const MAX: Order = Order(8);

    /// Iterater over all orders, from `Order::MIN` to `Order::MAX`
    fn orders() -> impl Iterator<Item = Order> {
        (Order::MIN.0..=Order::MAX.0).map(Order)
    }

    /// The number of frames in a block of this order
    fn block_frames(self) -> usize {
        1 << self.0
    }

    /// Calculates the size in bytes of the bitmap needed to track blocks of this order for a given number of frames. For example,
    /// the order-2 bitmap for 1024 frames would require 32 bytes.
    fn bitmap_size(self, frames: usize) -> usize {
        frames / self.block_frames() / 8
    }
}

struct Region {
    range: Range<PageFrame>
}

struct RegionContainer {
    link: LinkedListLink,

    region: Region, // TODO: spinlock?
}

impl RegionContainer {
    pub fn new(region: Region) -> RegionContainer {
        RegionContainer {
            link: LinkedListLink::new(),
            region
        }
    }
}

intrusive_adapter!(RegionAdapter = &'static RegionContainer : RegionContainer { link: LinkedListLink });

pub struct FrameAllocator {
    logger: KernelLogger,
    regions: LinkedList<RegionAdapter>
}

impl FrameAllocator {
    /// Creates a new `FrameAllocator` with no regions.
    pub fn new(logger: KernelLogger) -> FrameAllocator {
        FrameAllocator {
            logger,
            regions: LinkedList::new(RegionAdapter::new())
        }
    }

    /// Adds a region of physical memory to this allocator.
    /// 
    /// ## Unsafety
    /// The caller must ensure that `range` refers to a region of valid memory that is not already in use. Otherwise, allocations out of
    /// this range may overwrite arbitrary memory locations.
    pub unsafe fn add_region(&mut self, range: Range<PageFrame>) {
        let num_frames = range.end - range.start;
        info!(&self.logger, "Adding {:?} to frame allocator ({} frames)", range, num_frames);

        let metadata_size = mem::size_of::<RegionContainer>() + FrameAllocator::bitmaps_size(num_frames);
        debug!(&self.logger, "Reserving {}-byte metadata region", metadata_size; "start" => range.start);

        // TODO: need physical memory map to set up memory
        // also: have phys->virt and virt->phys mappers on PhysicalAddress and VirtualAddress because why not
    }

    /// The size (in bytes) of the frame allocator bitmaps for a region of `frames` frames.
    pub fn bitmaps_size(frames: usize) -> usize {
        Order::orders().map(|ord| ord.bitmap_size(frames)).sum()
    }
}
