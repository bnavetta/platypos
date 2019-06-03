use core::ptr::NonNull;

use acpi::{search_for_rsdp_bios, Acpi, AcpiHandler, PhysicalMapping};
use log::info;
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::{Page, PhysFrame, Mapper, PageTableFlags};
use x86_64::{PhysAddr, VirtAddr};

use crate::kernel_state;
use crate::util::page_count;

/// Starting address of the virtual address range where ACPI tables can be mapped in
const ACPI_MAP_START: u64 = 0xfffffa0000000000;
/// Number of pages to reserve for ACPI mappings
const ACPI_MAP_NPAGES: u64 = 64;

struct FixedRangeAcpiHandler {
    // TODO: reimplement on top of virtual address space manager
    address_range: PageRange,
}

impl FixedRangeAcpiHandler {
    fn new(start: Page, npages: u64) -> FixedRangeAcpiHandler {
        FixedRangeAcpiHandler {
            address_range: Page::range(start, start + npages)
        }
    }
}

impl AcpiHandler for FixedRangeAcpiHandler {
    fn map_physical_region<T>(
        &mut self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<T> {
        let phys_start = PhysFrame::containing_address(PhysAddr::new(physical_address as u64));
        let padding_start = physical_address - phys_start.start_address().as_u64() as usize;
        // Add any alignment to the region size
        let pages =
            page_count(size + padding_start);

        // Claim the next `pages` pages from our specified address range.
        let virtual_start = self
            .address_range
            .next()
            .expect("Could not allocate page for ACPI table mapping");
        for claimed in 1..pages {
            self.address_range
                .next()
                .expect("Could not allocate page for ACPI table mapping");
        }

        kernel_state().with_page_table(|table| {
            let mut frame_allocator = kernel_state().frame_allocator().page_table_allocator();
            let mut mapper = unsafe { table.active_4kib_mapper() };

            for i in 0..(pages as u64) {
                unsafe {
                    mapper
                        .map_to(
                            virtual_start + i,
                            phys_start + i,
                            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                            &mut frame_allocator,
                        )
                        .expect("Could not map ACPI table")
                        .flush()
                }
            }
        });

        let region_start = virtual_start.start_address().as_u64() as usize + padding_start;

        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: unsafe {
                NonNull::new_unchecked(region_start as *mut T)
            },
            region_length: pages * 4096 - padding_start,
            mapped_length: pages * 4096,
        }
    }

    fn unmap_physical_region<T>(&mut self, region: PhysicalMapping<T>) {
        assert_eq!(region.mapped_length % 4096, 0, "Mapping is not page-sized");
        let pages = region.region_length / 4096;

        // use containing_address instead of start_address because the region mapped in isn't necessarily page-aligned itself
        let virtual_start = Page::containing_address(VirtAddr::new(region.virtual_start.as_ptr() as usize as u64));

        kernel_state().with_page_table(|table| {
            let mut mapper = unsafe { table.active_4kib_mapper() };
            for i in 0..(pages as u64) {
                let (_, flush) = mapper
                    .unmap(virtual_start + i)
                    .expect("Could not unmap ACPI table");
                flush.flush();
            }
        })
    }
}

/// Discover and parse the ACPI tables, initializing kernel subsystems with the information found.
pub fn discover() {
    // Instead of storing all the ACPI information in a global variable, the ACPI subsystem calls other
    // subsystems with the relevant information. This avoids allocating unfreeable memory for data that's
    // only used on startup and makes the rest of the OS a bit less coupled to ACPI. For example,
    // the HPET could be initialized from any way of getting its base address.

    let mut handler = FixedRangeAcpiHandler::new(Page::from_start_address(VirtAddr::new(ACPI_MAP_START)).unwrap(), ACPI_MAP_NPAGES);

    let instance = unsafe { search_for_rsdp_bios(&mut handler) }.expect("Could not read ACPI tables");

    info!("Processors:");
    if let Some(processor) = instance.boot_processor() {
        info!("ID = {}, APIC ID = {}, state = {:?}", processor.processor_uid, processor.local_apic_id, processor.state);
    }
    for processor in instance.application_processors() {
        info!("ID = {}, APIC ID = {}, state = {:?}", processor.processor_uid, processor.local_apic_id, processor.state);
    }

    if let Some(interrupt_model) = instance.interrupt_model() {
        info!("Interrupt Model: {:?}", interrupt_model);
    }

    if let Some(hpet) = instance.hpet() {
        crate::time::hpet::init(PhysAddr::new(hpet.base_address as u64));
    }
}