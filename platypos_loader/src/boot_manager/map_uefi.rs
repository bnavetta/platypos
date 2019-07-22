use alloc::vec::Vec;

use log::{debug, info};
use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryAttribute, MemoryDescriptor, MemoryType};
use x86_64::structures::paging::{PageSize, PageTable, PageTableFlags, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use super::load_kernel::LoadKernel;
use super::util::{make_frame_range, make_page_range};
use super::{BootManager, Stage, KERNEL_DATA, KERNEL_IMAGE, KERNEL_PAGE_TABLE};

/// Stage 1: Mapping the UEFI environment
pub struct MapUefi;

impl Stage for MapUefi {
    type SystemTableView = Boot;
}

impl BootManager<MapUefi> {
    /// Create a new BootManager (stage 0) prepared for mapping the UEFI environment.
    pub fn new(
        system_table: SystemTable<Boot>,
        image_handle: uefi::Handle,
    ) -> BootManager<MapUefi> {
        let pml4_addr = system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, KERNEL_PAGE_TABLE, 1)
            .expect_success("Could not allocate PML4");

        let pml4 = unsafe { &mut *(pml4_addr as usize as *mut PageTable) };
        pml4.zero();

        BootManager {
            stage: MapUefi,
            system_table,
            image_handle,
            page_table: pml4,
            page_table_address: PhysAddr::new(pml4_addr),
        }
    }

    /// Transition from stage 1 to stage 2 by adding page table mappings for UEFI
    pub fn apply_memory_map(mut self) -> BootManager<LoadKernel> {
        let mut buf = vec![0u8; self.system_table.boot_services().memory_map_size()];
        let (_, memory_map) = self
            .system_table
            .boot_services()
            .memory_map(&mut buf)
            .log_warning()
            .expect("Could not get memory map");

        let mut regions: Vec<&MemoryDescriptor> = memory_map
            .filter(|desc| {
                // Only keep mappings needed for handing off. The kernel can otherwise access conventional
                // memory through its physical memory map.
                if desc.att.contains(MemoryAttribute::RUNTIME) {
                    true
                } else {
                    match desc.ty {
                        // It seems like the loader's stack is allocated as BOOT_SERVICES_DATA, so we have to keep it in the mapping
                        MemoryType::LOADER_CODE
                        | MemoryType::LOADER_DATA
                        | MemoryType::BOOT_SERVICES_DATA
                        | KERNEL_IMAGE
                        | KERNEL_DATA
                        | KERNEL_PAGE_TABLE => true,
                        _ => {
                            debug!("Skipping {:?}", desc);
                            false
                        }
                    }
                }
            })
            .collect();

        regions.sort_by_key(|desc| desc.phys_start);

        for region in regions.iter() {
            let phys_start = PhysAddr::new(region.phys_start);
            let size = region.page_count * 4096;

            debug!(
                "Identity-mapping {:?} {:#x} - {:#x} ({} bytes)",
                region.ty,
                phys_start,
                phys_start + size,
                size
            );

            assert_eq!(
                size % Size4KiB::SIZE,
                0,
                "Region is not an integer number of pages"
            );

            // We could probably use 2MiB pages for some of these, but the extra work isn't really
            // worth the efficiency gain since the kernel creates new page tables anyways

            self.map_contiguous(
                make_page_range(
                    VirtAddr::new(phys_start.as_u64()),
                    (size / Size4KiB::SIZE) as usize,
                ),
                make_frame_range(phys_start, (size / Size4KiB::SIZE) as usize),
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
            );
        }

        info!("Populated kernel page table with UEFI mappings");

        BootManager {
            stage: LoadKernel,
            system_table: self.system_table,
            image_handle: self.image_handle,
            page_table: self.page_table,
            page_table_address: self.page_table_address,
        }
    }
}
