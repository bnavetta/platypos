use core::fmt::Write;
use core::hint::unreachable_unchecked;
use core::mem;
use alloc::vec;

use log::info;

use uart_16550::SerialPort;

use uefi::prelude::*;
use uefi::Guid;
use uefi::table::boot::{MemoryAttribute, MemoryType, MemoryMapIter};

use x86_64::PhysAddr;
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging::{PageSize, Size4KiB, PhysFrame};
use x86_64_ext::paging::PhysFrameExt;
use x86_64_ext::instructions::hlt_loop;

use platypos_boot_info::BootInfo;
use platypos_boot_info::memory_map::{MemoryMap, MemoryRegion, MemoryKind, MAX_ENTRIES};

use crate::kernel_image::KernelImage;
use crate::page_table::KernelPageTable;
use crate::memory_map::{KERNEL_IMAGE, KERNEL_PAGE_TABLE, KERNEL_DATA, KERNEL_STACK_START, KERNEL_STACK_PAGES, create_boot_info, BOOT_INFO_ADDRESS};

/// Hands off to the kernel by exiting UEFI boot services and jumping to its entry point.
pub fn handoff(loaded_image: Handle, system_table: SystemTable<Boot>, kernel_image: &KernelImage, page_table: &mut KernelPageTable) -> ! {
    let mut debug_port = unsafe { SerialPort::new(0x3F8) };

    let boot_info = create_boot_info(page_table, system_table.boot_services());

    exit_boot_services(&mut debug_port, loaded_image, system_table, boot_info);
    unsafe {
        activate_page_table(&mut debug_port, page_table);
        jump_to_kernel(&mut debug_port, kernel_image);
    }
}

/// Exits UEFI boot services
fn exit_boot_services(debug_port: &mut SerialPort, loaded_image: Handle, system_table: SystemTable<Boot>, boot_info: *mut BootInfo) {
    // Add padding in case the memory map grows between now and calling exit_boot_services
    let mut memory_map_buf = vec![0u8; system_table.boot_services().memory_map_size() + 256];

    info!("Exiting UEFI boot services");

    let rsdp = find_rsdp(&system_table);

    debug_port.init();

    let (_, memory_map) = match system_table.exit_boot_services(loaded_image, &mut memory_map_buf) {
        Ok(completion) => {
            let (status, res) = completion.split();
            if status.is_success() {
                res
            } else {
                let _ = writeln!(debug_port, "Warning exiting UEFI boot services: {:?}", status);
                hlt_loop();
            }
        },
        Err(err) => {
            let _ = writeln!(debug_port, "Error exiting UEFI boot services: {:?}", err);
            hlt_loop();
        }
    };

    let _ = writeln!(debug_port, "Exited UEFI boot services");

    unsafe { *boot_info = BootInfo::new(rsdp, build_memory_map(debug_port, memory_map)) };

    let _ = writeln!(debug_port, "Populated boot info");

    // mem::forget to prevent calling the UEFI allocator after exiting boot services
    mem::forget(memory_map_buf);
}

// See section 5.2.5.2 of the UEFI ACPI specification, v6.2 (https://uefi.org/sites/default/files/resources/ACPI_6_2.pdf)

const ACPI_1_0_RSDP_GUID: Guid = Guid::from_values(0xeb9d2d30, 0x2d88, 0x11d3, 0x9a16, [0x00, 0x90, 0x27, 0x3f, 0xc1, 0x4d]);
const ACPI_2_0_RSDP_GUID: Guid = Guid::from_values(0x8868e871, 0xe4f1, 0x11d3, 0xbc22, [0x00, 0x80, 0xc7, 0x3c, 0x88, 0x81]); // technically 2.0+

fn find_rsdp(system_table: &SystemTable<Boot>) -> PhysAddr {
    for entry in system_table.config_table() {
        if entry.guid == ACPI_2_0_RSDP_GUID {
            return PhysAddr::new(entry.address as u64);
        }
    }

    for entry in system_table.config_table() {
        if entry.guid == ACPI_1_0_RSDP_GUID {
            return PhysAddr::new(entry.address as u64);
        }
    }

    panic!("ACPI RSDP not found in UEFI config table");
}

/// Builds a PlatypOS memory map out of the one provided by UEFI when we exited boot services
fn build_memory_map<'a>(debug_port: &mut SerialPort, uefi_map: MemoryMapIter<'a>) -> MemoryMap {
    let mut map = MemoryMap::new();

    for (i, descriptor) in uefi_map.enumerate() {
        // Check explicitly, because the panic handler relies on UEFI services
        if i >= MAX_ENTRIES {
            let _ = writeln!(debug_port, "Fatal error: more than {} entries in UEFI memory map", MAX_ENTRIES);
            hlt_loop();
        }

        let kind = if descriptor.att.contains(MemoryAttribute::RUNTIME) {
            MemoryKind::UefiRuntime
        } else {
            match descriptor.ty {
                KERNEL_IMAGE | KERNEL_DATA => MemoryKind::Kernel,
                KERNEL_PAGE_TABLE |
                MemoryType::LOADER_CODE |
                MemoryType::LOADER_DATA |
                MemoryType::BOOT_SERVICES_CODE |
                MemoryType::BOOT_SERVICES_DATA => MemoryKind::BootReclaimable,
                MemoryType::RUNTIME_SERVICES_CODE | MemoryType::RUNTIME_SERVICES_DATA => MemoryKind::UefiRuntime,
                MemoryType::CONVENTIONAL => MemoryKind::Conventional,
                MemoryType::ACPI_RECLAIM => MemoryKind::AcpiReclaimable,
                // other => MemoryKind::Other { uefi_type: other.0 }
                // Skip entries the kernel can't do anything useful with to save space
                _ => continue
            }
        };

        let frames = PhysFrame::containing_address(PhysAddr::new(descriptor.phys_start)).range_to(descriptor.page_count as usize);
        map.set_entry(i, MemoryRegion::new(frames, kind));
    }

    map.finish();
    map
}

/// Switches to the transitional kernel page table. The transitional page table must contain both mappings needed for the loader
/// and mappings needed for the kernel
unsafe fn activate_page_table(debug_port: &mut SerialPort, page_table: &KernelPageTable) {
    // Set the no-execute enable flag. Otherwise, switching to a page table using NO_EXECUTE bits will fail
    Efer::update(|efer| *efer |= EferFlags::NO_EXECUTE_ENABLE);

    let _ = writeln!(debug_port, "Activating kernel page table at {:?}", page_table.page_table_frame());
    Cr3::write(page_table.page_table_frame(), Cr3Flags::empty());
}

/// Jumps to the kernel's entry point
unsafe fn jump_to_kernel(debug_port: &mut SerialPort, kernel: &KernelImage) -> ! {
    let kernel_stack_pointer = KERNEL_STACK_START + KERNEL_STACK_PAGES * Size4KiB::SIZE as usize;

    let _ = writeln!(debug_port, "Jumping into kernel");
    llvm_asm!("pushq $0\n\t\
          retq\n\t\
          hlt" : : "r"(kernel.entry_address().as_u64()), "{rsp}"(kernel_stack_pointer), "{rdi}"(BOOT_INFO_ADDRESS) : "memory" : "volatile");
    unreachable_unchecked()
}
