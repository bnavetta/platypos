//! Kernel page table setup

use log::trace;
use uefi::prelude::*;
use uefi::table::boot::AllocateType;
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::KERNEL_RECLAIMABLE;

pub struct KernelPageTable {
    /// Kernel PML4 (top-level page table)
    page_table: &'static mut PageTable,

    /// Physical address of the kernel's PML4, needed so we can switch to it
    page_table_address: PhysAddr,
}

impl KernelPageTable {
    /// Creates a new, empty kernel page table.
    pub fn new(system_table: &SystemTable<Boot>) -> KernelPageTable {
        let page_table_address = system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, KERNEL_RECLAIMABLE, 1)
            .expect_success("Could not allocate kernel page table");
        trace!("Allocated kernel page table at {:0x}", page_table_address);

        // Safety: the firmware just told us we could use this
        let page_table = unsafe { &mut *(page_table_address as *mut PageTable) };
        page_table.zero();

        KernelPageTable {
            page_table_address: PhysAddr::new(page_table_address),
            page_table,
        }
    }

    /// Linearly maps `count` pages starting at `frame_start` in physical memory to `page_start` in the kernel's page table.
    pub fn map(
        &mut self,
        system_table: &SystemTable<Boot>,
        page_start: Page,
        frame_start: PhysFrame,
        count: usize,
        flags: PageTableFlags,
    ) {
        // This whole function isn't considered unsafe because we know the kernel page table isn't being used yet

        trace!(
            "Mapping {} pages starting at {:?} to {:?} (flags: {:?})",
            count,
            page_start,
            frame_start,
            flags
        );

        // Safety: we know the page table is valid and not in use. UEFI also guarantees that physical memory is identity-mapped
        let mut table = unsafe { OffsetPageTable::new(self.page_table, VirtAddr::new(0)) };
        let mut allocator = UefiFrameAllocator(system_table);

        for i in 0..count {
            let page = page_start + i as u64;
            let frame = frame_start + i as u64;
            // Safety: we know the page table is't in use, so this won't alias memory
            unsafe {
                // Using map_to_with_table_flags to make sure USER_ACCESSIBLE isn't set, and to set GLOBAL
                table
                    .map_to_with_table_flags(
                        page,
                        frame,
                        flags,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::GLOBAL,
                        &mut allocator,
                    )
                    .expect("Could not update kernel page table")
                    .ignore();
            }
        }
    }
}

struct UefiFrameAllocator<'a>(&'a SystemTable<Boot>);

unsafe impl<'a> FrameAllocator<Size4KiB> for UefiFrameAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        self.0
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, KERNEL_RECLAIMABLE, 1)
            .log_warning()
            .ok()
            .map(|start_addr| {
                PhysFrame::from_start_address(PhysAddr::new(start_addr))
                    .expect("Allocator returned an unaligned frame")
            })
    }
}
