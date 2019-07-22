#![no_std]
#![no_main]
#![feature(asm)]

#[macro_use]
extern crate alloc;

use core::fmt::Write;

use log::info;
use uefi::prelude::*;
use uefi::proto::console::text::Color;
use uefi::proto::loaded_image::LoadedImage;
use uefi::Handle;

mod filesystem;
mod loader;
mod util;

mod boot_manager;

use crate::boot_manager::BootManager;
use crate::util::to_string;

#[no_mangle]
pub extern "win64" fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utilities");
    system_table
        .stdout()
        .clear()
        .expect_success("Could not clear display");
    system_table
        .stdout()
        .set_color(Color::Green, Color::Black)
        .expect_success("Failed to set display colors");
    log::set_max_level(log::LevelFilter::Trace);

    info!("Welcome to the PlatypOS loader!");

    info!("Running on UEFI {:?}", system_table.uefi_revision());
    info!(
        "Firmware vendor: {}",
        to_string(system_table.firmware_vendor())
    );
    info!("Firmware version: {:?}", system_table.firmware_revision());

    let loaded_image = system_table
        .boot_services()
        .handle_protocol::<LoadedImage>(handle)
        .expect_success("Could not locate LoadedImage protocol");
    let loaded_image = unsafe { &*loaded_image.get() };
    info!("Loader image located at {:#x}", loaded_image.image_base());

    let boot_manager = BootManager::new(system_table, handle);
    boot_manager
        .apply_memory_map()
        .load_kernel()
        .exit_boot_services()
        .handoff();
}
