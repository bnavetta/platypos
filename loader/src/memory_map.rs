use alloc::vec;

use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryType, MemoryAttribute};

use log::debug;

use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{Page, PageTableFlags, PageSize, Size4KiB};
use x86_64::structures::paging::frame::PhysFrame;

use x86_64_ext::*;

use crate::page_table::KernelPageTable;

/// Memory type for the kernel image (both code and data)
pub const KERNEL_IMAGE: MemoryType = MemoryType(0x7000_0042);

/// Memory type for data allocated for the kernel by the bootloader, such as its stack
pub const KERNEL_DATA: MemoryType = MemoryType(0x7000_0043);

/// Memory type for the initial kernel page table created by the bootloader
pub const KERNEL_PAGE_TABLE: MemoryType = MemoryType(0x7000_0044);

/// Starting (low) address of the kernel stack
pub const KERNEL_STACK_START: VirtAddr = VirtAddr::new_truncate(0xffff_ffff_7100_000);

/// Kernel stack size, in 4KiB pages
pub const KERNEL_STACK_PAGES: usize = 4;

/// Add mappings needed by the UEFI environment to the kernel's page table. These mappings are only needed when
/// switching to the kernel page table while still inside the OS loader, but are not necessarily needed by the kernel itself.
pub fn map_uefi_environment(page_table: &mut KernelPageTable, boot_services: &BootServices) {
    let mut buf = vec![0u8; boot_services.memory_map_size()];
    let (_, memory_map) = boot_services.memory_map(&mut buf).expect_success("Could not get UEFI memory map");

    let regions = memory_map.filter(|desc| {
        desc.att.contains(MemoryAttribute::RUNTIME) || match desc.ty {
            // The loader's stack is allocated as BOOT_SERVICES_DATA rather than LOADER_DATA for some reason
            MemoryType::LOADER_CODE | MemoryType::LOADER_DATA | MemoryType::BOOT_SERVICES_DATA => true,
            _ => false
        }
    });

    for region in regions {
        // Not using region.virt_start, as it's always 0
        let pages = Page::containing_address(VirtAddr::new(region.phys_start)).range_to(region.page_count as usize);
        let frames = PhysFrame::containing_address(PhysAddr::new(region.phys_start)).range_to(region.page_count as usize);

        debug!("Adding {:?} to kernel page table", region);
        page_table.map(boot_services, pages, frames, PageTableFlags::PRESENT | PageTableFlags::WRITABLE);
    }
}

/// Allocates and maps the kernel's stack
pub fn create_kernel_stack(page_table: &mut KernelPageTable, boot_services: &BootServices) {
    let phys_start = boot_services
        .allocate_pages(AllocateType::AnyPages, KERNEL_DATA, KERNEL_STACK_PAGES)
        .expect_success("Could not allocate kernel stack");
    
    let pages = Page::from_start_address(KERNEL_STACK_START).unwrap().range_to(KERNEL_STACK_PAGES);
    let frames = PhysFrame::from_start_address(PhysAddr::new(phys_start)).unwrap().range_to(KERNEL_STACK_PAGES);
    page_table.map(boot_services, pages, frames, PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE);

    let stack = phys_start as usize as *mut u8;
    unsafe {
        stack.write_bytes(0, KERNEL_STACK_PAGES * Size4KiB::SIZE as usize);
    }

    debug!("Allocated {}-page kernel stack at {:#x}", KERNEL_STACK_PAGES, phys_start);
}