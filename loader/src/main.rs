#![no_std]
#![no_main]
#![feature(abi_efiapi)]

extern crate alloc;

use alloc::vec;
use core::mem;

use log::info;
use uefi::prelude::*;
use uefi::table::boot::MemoryType;
use uefi_services;

mod elf;
mod file;
mod load;
mod page_table;

use file::File;
use page_table::KernelPageTable;

/// Memory type for the kernel image
const KERNEL_IMAGE: MemoryType = MemoryType(0x7000_0042);

/// Memory type for loader-allocated kernel data
const KERNEL_DATA: MemoryType = MemoryType(0x7000_0043);

/// Memory type for loader-allocated kernel data that can be reclaimed, such as its initial page table
const KERNEL_RECLAIMABLE: MemoryType = MemoryType(0x7000_0044);

#[entry]
fn efi_main(image_handle: uefi::Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize UEFI services");

    let mut kernel_file = File::open(&system_table, "platypos_kernel");
    let kernel_image = elf::Object::from_file(&mut kernel_file);
    let mut page_table = KernelPageTable::new(&system_table);

    load::load_kernel_image(&system_table, &kernel_image, &mut kernel_file, &mut page_table);

    shutdown(image_handle, system_table);
}

/// Shut down the system cleanly
/// From https://github.com/rust-osdev/uefi-rs/blob/a7c3420527f80ba08b440b4607a43b047adf96b7/uefi-test-runner/src/main.rs#L104
fn shutdown(image_handle: uefi::Handle, system_table: SystemTable<Boot>) -> ! {
    use uefi::table::boot::MemoryDescriptor;
    use uefi::table::runtime::ResetType;

    // TODO: could stall for a couple seconds, at least if not QEMU
    info!("Shutting down...");

    let max_map_size =
        system_table.boot_services().memory_map_size() + 8 * mem::size_of::<MemoryDescriptor>();
    let mut map_storage = vec![0; max_map_size].into_boxed_slice();
    let (system_table, _) = system_table
        .exit_boot_services(image_handle, &mut map_storage[..])
        .expect_success("Failed to exit UEFI boot services");

    let runtime = unsafe { system_table.runtime_services() };
    runtime.reset(ResetType::Shutdown, Status::SUCCESS, None);
}
