//! Helpers useful across stages of booting.
use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryType};

use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{
    FrameAllocator, MappedPageTable, Mapper, Page, PageSize, PageTable, PageTableFlags, PhysFrame
};
use x86_64::{PhysAddr, VirtAddr};

use super::{BootManager, Stage, KERNEL_PAGE_TABLE};

/// Helpers which require boot services such as memory allocation.
impl<S: Stage> BootManager<S>
where
    S: Stage<SystemTableView = Boot>,
{
    /// Allocate `pages` pages of memory, of the specified type
    pub fn allocate_pages(&self, memory_type: MemoryType, pages: usize) -> Option<PhysAddr> {
        self.system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, memory_type, pages)
            .log_warning()
            .ok()
            .map(PhysAddr::new)
    }

    pub fn map_contiguous(
        &mut self,
        page_range: PageRange,
        frame_range: PhysFrameRange,
        flags: PageTableFlags,
    ) {
        assert_eq!(
            page_range.end - page_range.start,
            frame_range.end - frame_range.start,
            "Physical and virtual ranges differ in size"
        );

        let mut mapper = unsafe { MappedPageTable::new(self.page_table, identity_translator) };
        let mut allocator = UefiFrameAllocator::new(self.system_table.boot_services());

        for (page, frame) in page_range.zip(frame_range) {
            unsafe {
                mapper
                    .map_to(page, frame, flags, &mut allocator)
                    .expect("Could not add to kernel page table")
                    .ignore();
            }
        }
    }
}

/// Frame allocator that uses UEFI boot services to allocate pages
pub struct UefiFrameAllocator<'a> {
    boot_services: &'a BootServices,
}

impl<'a> UefiFrameAllocator<'a> {
    // This can't just be a method on BootManager because we need split borrows of the BootManager
    // struct - a borrow of the SystemTable for this and a borrow of the page table to map
    pub fn new(boot_services: &'a BootServices) -> UefiFrameAllocator<'a> {
        UefiFrameAllocator { boot_services }
    }
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
                    .expect("Allocated frame was not page-aligned")
            })
    }
}

/// Helper for making a PageRange given a starting address and number of pages
pub fn make_page_range<S: PageSize>(start_addr: VirtAddr, pages: usize) -> PageRange<S> {
    let page_start =
        Page::<S>::from_start_address(start_addr).expect("Start address was not page-aligned");
    Page::range(page_start, page_start + pages as u64)
}

/// Helper for making a PhysFrameRange given a starting address and number of frames
pub fn make_frame_range<S: PageSize>(start_addr: PhysAddr, frames: usize) -> PhysFrameRange<S> {
    let frame_start =
        PhysFrame::<S>::from_start_address(start_addr).expect("Start address was not page-aligned");
    PhysFrame::range(frame_start, frame_start + frames as u64)
}

/// Function for the MappedPageTable PhysToVirt closure
pub fn identity_translator(frame: PhysFrame) -> *mut PageTable {
    frame.start_address().as_u64() as usize as *mut PageTable
}
