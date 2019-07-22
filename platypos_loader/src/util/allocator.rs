use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryType};

use x86_64::structures::paging::{FrameAllocator, PageSize, PhysFrame};
use x86_64::PhysAddr;

/// Allocator for pages of memory using UEFI boot services
pub struct UefiPageAllocator<'a> {
    boot_services: &'a BootServices,
    page_table_memory_type: MemoryType,
}

impl<'a> UefiPageAllocator<'a> {
    pub fn new(
        boot_services: &'a BootServices,
        page_table_memory_type: MemoryType,
    ) -> UefiPageAllocator<'a> {
        UefiPageAllocator {
            boot_services,
            page_table_memory_type,
        }
    }

    pub fn allocate_pages(&mut self, memory_type: MemoryType, npages: usize) -> Option<PhysAddr> {
        self.boot_services
            .allocate_pages(AllocateType::AnyPages, memory_type, npages)
            .warning_as_error()
            .ok()
            .map(PhysAddr::new)
    }
}

unsafe impl<'a, S: PageSize> FrameAllocator<S> for UefiPageAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        let pages_needed = S::SIZE as usize / 4096; // UEFI page allocation API deals with 4KiB pages

        self.allocate_pages(self.page_table_memory_type, pages_needed)
            .map(|start| {
                PhysFrame::from_start_address(start).expect("Pages from UEFI allocator not aligned")
            })
    }
}
