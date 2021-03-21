#![no_std]
#![no_main]
#![feature(asm, abi_efiapi)]

extern crate alloc;

use alloc::vec;
use util::memory_map_size;

use log::info;
use uefi::prelude::*;
use uefi::table::Runtime;
use uefi::table::boot::MemoryType;
use uefi_services;

mod elf;
mod file;
mod page_table;
mod util;

use file::File;
use page_table::KernelPageTable;

/// Memory type for the kernel image
const KERNEL_IMAGE: MemoryType = MemoryType(0x7000_0042);

/// Memory type for loader-allocated kernel data
const KERNEL_DATA: MemoryType = MemoryType(0x7000_0043);

/// Memory type for loader-allocated kernel data that can be reclaimed, such as its initial page table
const KERNEL_RECLAIMABLE: MemoryType = MemoryType(0x7000_0044);

/// Size of one page of memory, defined here for convenience and to avoid magic numbers
const PAGE_SIZE: u64 = 4096;

#[entry]
fn uefi_start(image_handle: uefi::Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize UEFI services");
    log::set_max_level(log::LevelFilter::Trace);

    wait_for_debugger(&system_table, image_handle.clone());

    let kernel_file = File::open(&system_table, "platypos_kernel");
    let mut kernel_object = elf::Object::new(kernel_file);
    let mut page_table = KernelPageTable::new(&system_table);

    kernel_object.load_and_map(&system_table, &mut page_table);
    page_table.map_loader(&system_table);
    unsafe {
        exit_boot_services(image_handle, system_table);
        launch(&kernel_object, &page_table);
    }
}

/// Exits UEFI boot services. This is unsafe because the caller must ensure that no boot services are used after calling this function.
unsafe fn exit_boot_services(image_handle: uefi::Handle, system_table: SystemTable<Boot>) -> SystemTable<Runtime> {
    info!("Exiting UEFI boot services");
    let mut map_buf = vec![0u8; memory_map_size(&system_table)];
    let (runtime_table, _) = system_table.exit_boot_services(image_handle, &mut map_buf).expect_success("Failed to exit UEFI boot services");

    // We can't deallocate the memory map, because it was allocated using UEFI boot services that no longer exist
    ::core::mem::forget(map_buf);

    runtime_table
}

unsafe fn launch(kernel: &elf::Object, page_table: &KernelPageTable) -> ! {
    use x86_64::registers::{
        control::{Cr3, Cr3Flags},
        model_specific::{Efer, EferFlags}
    };

    // 1: Enable the no-execute bit, because the kernel page table uses it
    Efer::update(|flags| {
        *flags |= EferFlags::NO_EXECUTE_ENABLE;
    });

    // 2: Switch to the kernel page table
    Cr3::write(page_table.pml4_frame(), Cr3Flags::empty());

    // 3: Jump into the kernel!
    let entry: fn() -> ! = ::core::mem::transmute(kernel.metadata.entry());
    entry()
}

/// The GDB setup script will set this to 1 after it loads symbols.
static mut DEBUGGER_ATTACHED: u8 = 0;

fn wait_for_debugger(system_table: &SystemTable<Boot>, image_handle: uefi::Handle) {
    use uefi::proto::loaded_image::LoadedImage;
    let image = system_table.boot_services().handle_protocol::<LoadedImage>(image_handle).expect_success("Could not locate loaded image");
    let base_address = unsafe {
        let image = &* image.get();
        let (base_address, _size) = image.info();
        base_address
    };

    info!("Bootloader base address: {:#x}", base_address);
    info!("Waiting for debugger...");
    unsafe {
        while DEBUGGER_ATTACHED == 0 {
            asm!("pause",
              in("r13") base_address,
              options(nomem, nostack, preserves_flags)
            );
        }
    }
}