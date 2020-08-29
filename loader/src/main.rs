#![no_std]
#![no_main]
#![feature(llvm_asm)]

extern crate alloc;

use log::info;
use uefi::prelude::*;
use uefi::proto::console::text::Color;

use uefi_services;

mod handoff;
mod kernel_image;
mod memory_map;
mod page_table;
mod util;

use crate::kernel_image::KernelImage;
use crate::page_table::KernelPageTable;

#[no_mangle]
pub extern "win64" fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utilities");

    // Clear the screen
    system_table
        .stdout()
        .reset(false)
        .expect_success("Could not reset screen");

    system_table
        .stdout()
        .set_color(Color::White, Color::Black)
        .expect_success("Could not set console colors");

    info!("Welcome to the PlatypOS bootloader!");

    info!(
        "Running on UEFI revision {:?}",
        system_table.uefi_revision()
    );

    info!(
        "Firmware {} {:?}",
        system_table.firmware_vendor(),
        system_table.firmware_revision()
    );

    let boot_services = system_table.boot_services();

    let mut page_table = KernelPageTable::new(boot_services);

    let mut kernel = KernelImage::open(boot_services);
    kernel.load(boot_services, &mut page_table);

    memory_map::create_kernel_stack(&mut page_table, boot_services);
    memory_map::map_uefi_environment(&mut page_table, boot_services);
    handoff::handoff(handle, system_table, &kernel, &mut page_table);
}
