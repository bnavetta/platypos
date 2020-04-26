#![no_std]
#![no_main]
#![feature(llvm_asm)]

extern crate alloc;

use log::{self, info, LevelFilter};
use uefi::prelude::*;

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
    log::set_max_level(LevelFilter::Trace);

    // Clear the screen
    system_table
        .stdout()
        .reset(false)
        .expect_success("Could not reset screen");

    info!(
        "Running on UEFI revision {:?}",
        system_table.uefi_revision()
    );

    let boot_services = system_table.boot_services();

    let mut page_table = KernelPageTable::new(boot_services);

    let mut kernel = KernelImage::open(boot_services);
    kernel.load(boot_services, &mut page_table);

    memory_map::create_kernel_stack(&mut page_table, boot_services);
    memory_map::map_uefi_environment(&mut page_table, boot_services);
    handoff::handoff(handle, system_table, &kernel, &page_table);
}
