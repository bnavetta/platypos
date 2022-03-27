//! Kernel page table setup

use alloc::vec;

use log::{debug, trace};
use uefi::table::boot::AllocateType;
use uefi::{prelude::*, ResultExt};
use x86_64::{
    structures::paging::{
        FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame,
        Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::util::{allocate_frames, memory_map_size};
use crate::{KERNEL_DATA, KERNEL_RECLAIMABLE, PAGE_SIZE};

pub struct KernelPageTable {
    /// Kernel PML4 (top-level page table)
    page_table: &'static mut PageTable,

    /// Physical address of the kernel's PML4, needed so we can switch to it
    page_table_address: PhysAddr,
}

impl KernelPageTable {
    /// Creates a new, empty kernel page table.
    pub fn new(system_table: &SystemTable<Boot>) -> KernelPageTable {
        let (page_table_address, _) = allocate_frames(system_table, 1, KERNEL_RECLAIMABLE);
        trace!("Allocated kernel page table at {:0x}", page_table_address);

        // Safety: the firmware just told us we could use this
        let page_table = unsafe { &mut *(page_table_address.as_u64() as *mut PageTable) };
        page_table.zero();

        KernelPageTable {
            page_table_address,
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
            let page: Page = page_start + (i as u64);
            let frame: PhysFrame = frame_start + (i as u64);
            // Safety: we know the page table is't in use, so this won't alias memory
            unsafe {
                // Using map_to_with_table_flags to make sure USER_ACCESSIBLE isn't set, and to set GLOBAL
                table
                    .map_to_with_table_flags(
                        page,
                        frame,
                        flags,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        &mut allocator,
                    )
                    .expect("Could not update kernel page table")
                    .ignore();
            }
        }
    }

    /// Adds mappings for the boot loader. This lets us switch to the kernel's page table before jumping into the kernel
    pub fn map_loader(&mut self, system_table: &SystemTable<Boot>) {
        use uefi::table::boot::MemoryType;

        let mut map_storage = vec![0u8; memory_map_size(system_table)];
        let (_, memory_map) = system_table
            .boot_services()
            .memory_map(&mut map_storage)
            .expect_success("Could not retrieve UEFI memory map");
        for desc in memory_map {
            // TODO: also map runtime services?
            let flags = match desc.ty {
                MemoryType::LOADER_CODE => PageTableFlags::PRESENT,
                // We have to keep BOOT_SERVICES_DATA around, since that contains the bootloader stack
                MemoryType::LOADER_DATA
                | MemoryType::BOOT_SERVICES_DATA
                | KERNEL_DATA
                | KERNEL_RECLAIMABLE => {
                    PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE
                }
                _ => {
                    debug!(
                        "Skipping {:?} at {:#x} - {:#x}",
                        desc.ty,
                        desc.phys_start,
                        desc.phys_start + desc.page_count * PAGE_SIZE
                    );
                    continue;
                }
            };

            debug!(
                "Adding loader mapping for {:?} at {:#x} - {:#x}",
                desc.ty,
                desc.phys_start,
                desc.phys_start + desc.page_count * PAGE_SIZE
            );

            let page_start = Page::from_start_address(VirtAddr::new(if desc.virt_start == 0 {
                // In practice, it seems like virt_start is always 0, so identity-map at the physical address
                desc.phys_start
            } else {
                desc.virt_start
            }))
            .expect("Unaligned memory descriptor");
            let frame_start = PhysFrame::from_start_address(PhysAddr::new(desc.phys_start))
                .expect("Unaligned memory descriptor");

            self.map(
                system_table,
                page_start,
                frame_start,
                desc.page_count as usize,
                flags,
            );
        }
    }

    /// The physical frame of memory containing the top-level page table.
    pub fn pml4_frame(&self) -> PhysFrame {
        PhysFrame::from_start_address(self.page_table_address).expect("Invalid PML4 address")
    }
}

struct UefiFrameAllocator<'a>(&'a SystemTable<Boot>);

unsafe impl<'a> FrameAllocator<Size4KiB> for UefiFrameAllocator<'a> {
    fn allocate_frame(&mut self) -> Option<PhysFrame> {
        // TODO: consider allocating a pool of frames and using that, so all the initial kernel page table memory is in one place
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
