use core::sync::atomic::{AtomicUsize, Ordering};

use crate::allocators::physical::PHYSICAL_ALLOCATOR;
use crate::platform::{PhysicalAddress, VirtualAddress};

/// The size of a physical page frame, in bytes
pub const FRAME_SIZE: usize = 4096;

/// Stores the offset of the physical memory map the bootloader creates
static PHYSICAL_MEMORY_MAP_START: AtomicUsize = AtomicUsize::new(0);

pub fn init() {
//    PHYSICAL_MEMORY_MAP_START.store(boot_info.physical_memory_offset as usize, Ordering::Relaxed);
//
//    let mut frame_allocator = PHYSICAL_ALLOCATOR.lock();
//    for region in boot_info.memory_map.iter() {
//        if region.region_type == MemoryRegionType::Usable {
//            frame_allocator.add_range(
//                region.range.start_addr().into(),
//                region.range.end_addr().into(),
//            );
//        }
//    }
}

/// Get a virtual address which can be used to access the given physical address. This relies on
/// the bootloader mapping all physical memory into the kernel address space
pub fn physical_to_virtual(addr: PhysicalAddress) -> VirtualAddress {
    let offset = PHYSICAL_MEMORY_MAP_START.load(Ordering::Relaxed);
    debug_assert_ne!(offset, 0, "Physical memory map location not set");
    VirtualAddress::new(addr.as_usize() + offset)
}
