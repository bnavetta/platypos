use alloc::vec;
use alloc::vec::Vec;
use core::cmp::max;

use log::{debug, info};
use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryAttribute, MemoryDescriptor, MemoryType};
use x86_64::structures::paging::{
    MappedPageTable, Mapper, Page, PageSize, PageTable, PageTableFlags, PhysFrame,
    Size2MiB, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

use super::load_kernel::LoadKernel;
use super::util::{identity_translator, UefiFrameAllocator};
use super::{BootManager, Stage};

/// Stage 1: Mapping the UEFI environment
pub struct MapUefi;

impl Stage for MapUefi {
    type SystemTableView = Boot;
}

impl BootManager<MapUefi> {
    /// Create a new BootManager (stage 0) prepared for mapping the UEFI environment.
    pub fn new(system_table: SystemTable<Boot>) -> BootManager<MapUefi> {
        let pml4_addr = system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, MemoryType::LOADER_DATA, 1)
            .expect_success("Could not allocate PML4");

        let pml4 = unsafe { &mut *(pml4_addr as usize as *mut PageTable) };
        pml4.zero();

        BootManager {
            stage: MapUefi,
            system_table,
            page_table: pml4,
            page_table_address: PhysAddr::new(pml4_addr),
        }
    }

    /// Transition from stage 1 to stage 2 by adding page table mappings for UEFI
    pub fn apply_memory_map(self) -> BootManager<LoadKernel> {
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
                        MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => true,
                        _ => false,
                    }
                }
            })
            .collect();

        regions.sort_by_key(|desc| desc.phys_start);

        let mut mapper = unsafe { MappedPageTable::new(self.page_table, identity_translator) };
        let mut allocator = UefiFrameAllocator::new(self.system_table.boot_services());

        for region in regions.iter() {
            let mut start = PhysAddr::new(region.phys_start);
            let mut remaining = region.page_count * 4096;

            // If it's a large enough region, round the starting address to the nearest 2MiB so we
            // can use huge pages for efficiency
            if remaining >= Size2MiB::SIZE {
                start = start.align_down(Size2MiB::SIZE);
                remaining += region.phys_start - start.as_u64();
            }

            debug!(
                "Identity-mapping {:?} {:#x} - {:#x} ({} bytes)",
                region.ty,
                start,
                start + remaining,
                remaining
            );

            while remaining >= Size2MiB::SIZE {
                let page = Page::<Size2MiB>::from_start_address(VirtAddr::new(start.as_u64()))
                    .expect("Region not aligned to a 2MiB boundary");
                let frame = PhysFrame::from_start_address(start).unwrap();

                unsafe {
                    mapper
                        .map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut allocator,
                        )
                        .expect("Could not map region")
                        .ignore();
                }

                remaining -= Size2MiB::SIZE;
                start += Size2MiB::SIZE;
            }

            while remaining >= Size4KiB::SIZE {
                let page = Page::<Size4KiB>::from_start_address(VirtAddr::new(start.as_u64()))
                    .expect("Region not aligned to a 4KiB boundary");
                let frame = PhysFrame::from_start_address(start).unwrap();

                unsafe {
                    mapper
                        .map_to(
                            page,
                            frame,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut allocator,
                        )
                        .expect("Could not map region")
                        .ignore();
                }

                remaining -= Size4KiB::SIZE;
                start += Size4KiB::SIZE;
            }

            assert_eq!(remaining, 0, "Region is not an integral number of pages");
        }

        info!("Populated kernel page table with UEFI mappings");

        BootManager {
            stage: LoadKernel,
            system_table: self.system_table,
            page_table: self.page_table,
            page_table_address: self.page_table_address,
        }
    }

    /// Returns the amount of physical memory the system has, usable or otherwise
    fn phys_mem_size(&self) -> u64 {
        let mut max_addr: u64 = 0;

        let mut buf = vec![0u8; self.system_table.boot_services().memory_map_size()];
        let (_, memory_map) = self
            .system_table
            .boot_services()
            .memory_map(&mut buf)
            .log_warning()
            .expect("Could not get memory map");

        for descriptor in memory_map {
            // Skip memory types that don't correspond to actual physical memory
            if descriptor.ty == MemoryType::MMIO || descriptor.ty == MemoryType::RESERVED {
                continue;
            }
            max_addr = max(
                max_addr,
                descriptor.phys_start + descriptor.page_count * 4096,
            );
        }

        // Since max_addr is address right after end of region, it'll be the amount of physical memory
        max_addr
    }

    /*

    /// Identity-map all of physical memory, so that UEFI services are accessible in the kernel
    /// address space.
    fn map_physical(&mut self) {
        let mem_size = self.phys_mem_size();
        debug!("Detected {} bytes of physical memory", mem_size);

        let pages_needed = (mem_size + Size1GiB::SIZE - 1) / Size1GiB::SIZE;

        let page_start = Page::<Size1GiB>::containing_address(VirtAddr::new(0));
        let frame_start = PhysFrame::containing_address(PhysAddr::new(0));

        let mut mapper = unsafe { MappedPageTable::new(self.page_table, identity_translator) };
        let mut allocator = UefiFrameAllocator::new(self.system_table.boot_services());

        for i in 0..pages_needed {
            unsafe {
                mapper
                    .map_to(
                        page_start + i,
                        frame_start + i,
                        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                        &mut allocator,
                    )
                    .expect("Could not map physical memory")
                    .ignore()
            };
        }

        debug!("Identity-mapped physical memory");
    }

    fn map_mmio(&mut self) {
        // add MMIO mappings we ignored earlier (use 2MiB and 4KiB)

        // TODO: need max phys addr, in case there's MMIO below that to skip
        // OR do a single pass, mapping MMIO as encountered?
    }

    /// Transition from stage 1 to stage 2 by adding page table mappings for UEFI
    pub fn create_uefi_mappings(mut self) -> BootManager<LoadKernel> {
        self.map_physical();
        self.map_mmio();

        BootManager {
            stage: LoadKernel,
            system_table: self.system_table,
            page_table: self.page_table,
            page_table_address: self.page_table_address,
        }
    }

    */
}
