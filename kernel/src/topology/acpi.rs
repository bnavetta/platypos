use core::ptr::NonNull;

use acpi::{search_for_rsdp_bios, AcpiHandler, PhysicalMapping};
use log::info;
use x86_64::PhysAddr;

use crate::memory::physical_to_virtual;

/// AcpiHandler implementation using the kernel's physical memory mapping
struct KernelAcpiHandler;

impl AcpiHandler for KernelAcpiHandler {
    fn map_physical_region<T>(
        &mut self,
        physical_address: usize,
        size: usize,
    ) -> PhysicalMapping<T> {
        PhysicalMapping {
            physical_start: physical_address,
            virtual_start: unsafe {
                NonNull::new_unchecked(
                    physical_to_virtual(PhysAddr::new(physical_address as u64)).as_mut_ptr(),
                )
            },
            region_length: size,
            mapped_length: size,
        }
    }

    fn unmap_physical_region<T>(&mut self, _region: PhysicalMapping<T>) {
        // Nothing to do!
    }
}

/// Discover and parse the ACPI tables, initializing kernel subsystems with the information found.
pub fn discover() {
    // Instead of storing all the ACPI information in a global variable, the ACPI subsystem calls other
    // subsystems with the relevant information. This avoids allocating unfreeable memory for data that's
    // only used on startup and makes the rest of the OS a bit less coupled to ACPI. For example,
    // the HPET could be initialized from any way of getting its base address.

    let mut handler = KernelAcpiHandler;

    let instance =
        unsafe { search_for_rsdp_bios(&mut handler) }.expect("Could not read ACPI tables");

    crate::topology::processor::init(instance.boot_processor(), instance.application_processors());

    if let Some(interrupt_model) = instance.interrupt_model() {
        info!("Interrupt Model: {:?}", interrupt_model);
    }

    if let Some(hpet) = instance.hpet() {
        crate::time::hpet::init(PhysAddr::new(hpet.base_address as u64));
    }
}
