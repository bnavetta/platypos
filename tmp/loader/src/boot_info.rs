//! Builds up boot info to pass to the kernel

use core::{mem::{self, MaybeUninit}, slice};

use log::info;
use platypos_boot_info::{BootInfo, MemoryKind, MemoryRegion};
use uefi::{prelude::*, table::boot::{MemoryDescriptor, MemoryType}, Guid};
use x86_64::PhysAddr;

use crate::{KERNEL_RECLAIMABLE, KERNEL_IMAGE, KERNEL_DATA, PAGE_SIZE};
use crate::util::allocate_frames;

/// Builder for boot info. This must be created before the kernel's page table is configured and finalized after exiting UEFI boot services.
pub struct BootInfoBuilder {
    /// Page of memory containing boot information. The [`BootInfo`] struct is at the start of the page, followed by the memory map array.
    info_page: PhysAddr,
    rsdp: Option<PhysAddr>,
}

const MAX_MEMORY_MAP_ENTRIES: usize = (PAGE_SIZE as usize - mem::size_of::<BootInfo>()) / mem::size_of::<MemoryRegion>();

impl BootInfoBuilder {
    pub fn new(system_table: &SystemTable<Boot>) -> BootInfoBuilder {
        let (frame, _) = allocate_frames(system_table, 1, KERNEL_DATA);
        info!("Allocated boot info page at {:#x}", frame.as_u64());
        BootInfoBuilder {
            info_page: frame,
            rsdp: BootInfoBuilder::find_rsdp(system_table),
        }
    }

    /// Finalize [`BootInfo`] generation. This must be called with the final memory map after exiting UEFI boot services.
    pub fn generate<'a>(self, memory_descriptors: impl Iterator<Item = &'a MemoryDescriptor>) -> &'static BootInfo {
        let info_pointer = self.info_page.as_u64() as *mut MaybeUninit<BootInfo>;
        // I don't think this handles alignment correctly
        let memory_map_pointer = unsafe { info_pointer.add(1) }.cast::<MaybeUninit<MemoryRegion>>();

        let memory_map: &'static mut [MaybeUninit<MemoryRegion>] = unsafe {
            slice::from_raw_parts_mut(memory_map_pointer, MAX_MEMORY_MAP_ENTRIES)
        };

        let memory_map = BootInfoBuilder::fill_memory_map(memory_descriptors, memory_map);
        unsafe {
            *info_pointer = MaybeUninit::new(BootInfo::new(self.rsdp, memory_map));
            info_pointer.as_ref().unwrap().assume_init_ref()
        }
    }

    fn fill_memory_map<'a, 'b>(memory_descriptors: impl Iterator<Item = &'a MemoryDescriptor>, memory_map: &'b mut [MaybeUninit<MemoryRegion>]) -> &'b [MemoryRegion] {
        let mut regions = memory_descriptors.filter_map(|descriptor| {
            let kind = match descriptor.ty {
                MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => MemoryKind::Usable,
                MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => MemoryKind::Usable,
                MemoryType::CONVENTIONAL => MemoryKind::Usable,
                MemoryType::ACPI_RECLAIM => MemoryKind::AcpiTables,
                MemoryType::PERSISTENT_MEMORY => MemoryKind::NonVolatile,
                MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => MemoryKind::UefiRuntime,
                KERNEL_RECLAIMABLE => MemoryKind::KernelReclaimable,
                KERNEL_IMAGE | KERNEL_DATA => MemoryKind::Kernel,
                _ => return None,
            };

            let start = PhysAddr::new(descriptor.phys_start);
            let end = start + descriptor.page_count * PAGE_SIZE;
            Some(MemoryRegion::new(start, end, kind))
        });

        // TODO: coalesce adjacent regions of the same kind
        // Can probably figure the memory descriptors are sorted and combine adjacent entries, instead of pre-sorting

        let mut idx = 0;
        let prev_region = regions.next().expect("empty memory map!");
        let last_region = regions.fold(prev_region, |prev_region, next_region| {
            if prev_region.kind() == next_region.kind() && prev_region.end() >= next_region.start() {
                // coalesce, and return coalesced region
                MemoryRegion::new(prev_region.start(), next_region.end(), prev_region.kind())
            } else {
                memory_map[idx] = MaybeUninit::new(prev_region);
                idx += 1;
                next_region
                // push prev region to list
                // return next region
            }
        });

        memory_map[idx] = MaybeUninit::new(last_region);

        // let mut len = 0;
        // for (idx, region) in regions.enumerate() {
        //     memory_map[idx] = MaybeUninit::new(region);
        //     len += 1;
        // }

        // Safety: we know that the first `len` entries of the array are initialized
        unsafe { MaybeUninit::slice_assume_init_ref(&memory_map[0..=idx]) }
    }

    /// Attempt to locate the ACPI RSDP
    fn find_rsdp(system_table: &SystemTable<Boot>) -> Option<PhysAddr> {
        for entry in system_table.config_table() {
            if entry.guid == ACPI_2_0_RSDP_GUID || entry.guid == ACPI_1_0_RSDP_GUID {
                // TODO: should this indicate which version it is?
                return Some(PhysAddr::new(entry.address as u64))
            }
        }
        None
    }
}

// See section 5.2.5.2 of the UEFI ACPI specification, v6.2 (https://uefi.org/sites/default/files/resources/ACPI_6_2.pdf)
const ACPI_1_0_RSDP_GUID: Guid = Guid::from_values(
    0xeb9d2d30,
    0x2d88,
    0x11d3,
    0x9a16,
    [0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d],
);
const ACPI_2_0_RSDP_GUID: Guid = Guid::from_values(
    0x8868e871,
    0xe4f1,
    0x11d3,
    0xbc22,
    [0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81],
); // technically 2.0+

