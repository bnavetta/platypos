use uefi::prelude::*;
use uefi::table::boot::AllocateType;

use log::trace;

use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, PageSize, PageTable, PageTableFlags, PhysFrame,
};
use x86_64::{PhysAddr, VirtAddr};

use crate::memory_map::KERNEL_PAGE_TABLE;

/// Kernel's page table, which is gradually filled in during OS loading
pub struct KernelPageTable {
    /// Kernel PML4 structure
    page_table: &'static mut PageTable,

    /// Physical address of the kernel PML4
    page_table_start: PhysAddr,
}

impl KernelPageTable {
    /// Allocates a new, empty kernel page table
    pub fn new(boot_services: &BootServices) -> KernelPageTable {
        let pml4_address = boot_services
            .allocate_pages(AllocateType::AnyPages, KERNEL_PAGE_TABLE, 1)
            .expect_success("Could not allocate kernel PML4");

        let pml4 = unsafe { &mut *(pml4_address as usize as *mut PageTable) };
        pml4.zero();
        KernelPageTable {
            page_table: pml4,
            page_table_start: PhysAddr::new(pml4_address),
        }
    }

    /// Physical frame containing the kernel's PML4
    pub fn page_table_frame(&self) -> PhysFrame {
        PhysFrame::from_start_address(self.page_table_start)
            .expect("Kernel PML4 location is not page-aligned")
    }

    /// Maps a range of physical frames to pages in the kernel address space
    pub fn map(
        &mut self,
        boot_services: &BootServices,
        pages: PageRange,
        frames: PhysFrameRange,
        flags: PageTableFlags,
    ) {
        assert_eq!(
            pages.end - pages.start,
            frames.end - frames.start,
            "Physical and virtual ranges are not the same size"
        );

        trace!(
            "Mapping {} pages starting at {:?} to {:?} with {:?}",
            pages.end - pages.start,
            pages.start,
            frames.start,
            flags
        );

        let mut mapped_table = unsafe { OffsetPageTable::new(self.page_table, VirtAddr::new(0)) };
        let mut allocator = UefiFrameAllocator { boot_services };

        for (page, frame) in pages.zip(frames) {
            unsafe {
                mapped_table
                    .map_to(page, frame, flags, &mut allocator)
                    .expect("Could not update kernel page table")
                    .ignore();
            }
        }
    }
}

/// Frame allocator using the UEFI boot services page allocator
struct UefiFrameAllocator<'a> {
    boot_services: &'a BootServices,
}

unsafe impl<'a, S: PageSize> FrameAllocator<S> for UefiFrameAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        let frames_needed = S::SIZE / 4096;
        self.boot_services
            .allocate_pages(
                AllocateType::AnyPages,
                KERNEL_PAGE_TABLE,
                frames_needed as usize,
            )
            .log_warning()
            .ok()
            .map(|start_addr| {
                PhysFrame::from_start_address(PhysAddr::new(start_addr))
                    .expect("UEFI boot services returned an unaligned frame")
            })
    }
}
