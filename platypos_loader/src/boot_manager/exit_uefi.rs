use core::fmt::Write;
use core::mem;

use log::{debug, info};
use uart_16550::SerialPort;
use uefi::prelude::*;
use uefi::table::boot::{MemoryAttribute, MemoryType, MemoryMapIter};
use x86_64::{VirtAddr, PhysAddr};
use x86_64::structures::paging::PageTableFlags;

use platypos_boot_info::BootInfo;
use platypos_boot_info::memory_map::{MemoryMap, MemoryRegion, MemoryKind, MAX_ENTRIES};

use super::{BootManager, Stage, BOOT_INFO_ADDR, KERNEL_DATA, KERNEL_IMAGE, KERNEL_PAGE_TABLE};
use super::handoff::Handoff;
use super::util::{make_page_range, make_frame_range};
use crate::util::halt_loop;

pub struct ExitUefiBootServices {
    /// Address of the kernel entry point
    pub kernel_entry_addr: VirtAddr,
}

impl Stage for ExitUefiBootServices {
    type SystemTableView = Boot;
}

impl BootManager<ExitUefiBootServices> {
    pub fn exit_boot_services(mut self) -> BootManager<Handoff> {
        // Allocate and map the boot info while we still have boot services
        let boot_info_addr = self.allocate_pages(KERNEL_DATA, 1).expect("Could not allocate memory for boot info");
        let boot_info = unsafe { &mut * (boot_info_addr.as_u64() as usize as *mut BootInfo) };
        self.map_contiguous(
            make_page_range(VirtAddr::new(BOOT_INFO_ADDR), 1),
            make_frame_range(boot_info_addr, 1),
            PageTableFlags::PRESENT,
        );

        // Add some padding in case the memory map changes size
        let memory_map_size = self.system_table.boot_services().memory_map_size() + 256;
        debug!("Allocating {} bytes for final memory map", memory_map_size);
        let mut memory_map_buffer = vec![0u8; memory_map_size];

        info!("Exiting UEFI boot services");

        let mut debug_port = unsafe { SerialPort::new(0x3F8) };
        debug_port.init();

        let (table, uefi_map) = match self
            .system_table
            .exit_boot_services(self.image_handle, &mut memory_map_buffer)
        {
            Ok(comp) => {
                let (status, res) = comp.split();
                if status.is_success() {
                    res
                } else {
                    writeln!(
                        &mut debug_port,
                        "Warning exiting boot services: {:?}",
                        status
                    ).unwrap();
                    halt_loop();
                }
            }
            Err(err) => {
                writeln!(&mut debug_port, "Error exiting boot services: {:?}", err).unwrap();
                halt_loop();
            }
        };

        writeln!(&mut debug_port, "Exited UEFI boot services").unwrap();

        populate_boot_info(boot_info, uefi_map, &mut debug_port);

        // Can't deallocate it since we no longer have an allocator
        mem::forget(memory_map_buffer);

        BootManager {
            stage: Handoff {
                kernel_entry_addr: self.stage.kernel_entry_addr,
                debug_port,
            },
            system_table: table,
            image_handle: self.image_handle,
            page_table: self.page_table,
            page_table_address: self.page_table_address,
        }
    }
}

fn populate_boot_info(boot_info: &mut BootInfo, uefi_map: MemoryMapIter, debug_port: &mut SerialPort) {
    let mut i = 0;
    let mut memory_map = MemoryMap::new();
    for descriptor in uefi_map {
        let uefi_required = descriptor.att.contains(MemoryAttribute::RUNTIME);

        let kind = match descriptor.ty {
            KERNEL_IMAGE | KERNEL_DATA=> MemoryKind::Kernel,
            KERNEL_PAGE_TABLE => MemoryKind::KernelPageTable,
            MemoryType::LOADER_CODE | MemoryType::LOADER_DATA => MemoryKind::Bootloader,
            MemoryType::BOOT_SERVICES_CODE | MemoryType::BOOT_SERVICES_DATA => MemoryKind::BootServices,
            MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => MemoryKind::RuntimeServices,
            MemoryType::CONVENTIONAL => MemoryKind::Conventional,
            MemoryType::ACPI_RECLAIM => MemoryKind::AcpiReclaimable,
            other => if uefi_required { MemoryKind::Other { uefi_type: other.0 } } else { continue }
        };

        let region = MemoryRegion::new(kind, uefi_required, PhysAddr::new(descriptor.phys_start), descriptor.page_count as usize);
        if i >= MAX_ENTRIES {
            writeln!(debug_port, "Exceeded maximum number of memory map entries").unwrap();
            loop {}
        }
        memory_map.set_entry(i, region);
        i += 1
    }

    *boot_info = BootInfo::new(memory_map);
}