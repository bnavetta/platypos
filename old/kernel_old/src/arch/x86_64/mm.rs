//! x86_64-specific memory management

use slog::{o, debug};

use x86_64::structures::paging::frame::{PhysFrame, PhysFrameRange};

use platypos_boot_info::memory_map::{MemoryKind, MemoryMap, MemoryUsability};

use crate::mm::address::PageFrame;
use crate::mm::frame::FrameAllocator;
use crate::KernelLogger;

/// Size of a page of virtual memory or frame of physical memory
pub const PAGE_SIZE: usize = 4096;

pub fn initialize_frame_allocator(logger: &KernelLogger, map: &MemoryMap) {
    debug!(logger, "Initializing frame allocator");
    // Calls to alloc.add_region are safe because we're adding them based on the firmware-provided memory map.

    let mut alloc = FrameAllocator::new(logger.new(o!()));

    let region = map.iter().filter(|region| {
        region.kind() == MemoryKind::Conventional
            && (region.usability() == MemoryUsability::Usable
                || region.usability() == MemoryUsability::BootReclaimable)
    })
    .map(|region| region.range())
    .fold(None, |prev: Option<PhysFrameRange>, region| match prev {
        Some(prev) => if prev.end == region.start {
            Some(PhysFrame::range(prev.start, region.end))
        } else {
            unsafe { alloc.add_region(prev.start.into()..prev.end.into()) };
            Some(region)
        },
        None => Some(region),
    });

    // Add the last physical memory region
    if let Some(region) = region {
        unsafe { alloc.add_region(region.start.into()..region.end.into()) };
    }
}

impl Into<PageFrame> for PhysFrame {
    fn into(self) -> PageFrame {
        PageFrame::new(self.start_address().as_u64() as usize / PAGE_SIZE)
    }
}