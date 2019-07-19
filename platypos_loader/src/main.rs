#![no_std]
#![no_main]
#![feature(asm)]

extern crate alloc;

use log::info;
use uefi::prelude::*;
use uefi::proto::console::text::Color;
use uefi::Handle;

mod filesystem;
mod loader;
mod util;

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

    let rev = system_table.uefi_revision();
    info!("Running on UEFI {}.{}", rev.major(), rev.minor());

    loader::launch_kernel(handle, system_table, &["platypos_kernel"]);

    loop {}
}
