#![no_std]
#![no_main]

use log::info;
use uefi::prelude::*;

use uefi_services;

#[no_mangle]
pub extern "win64" fn efi_main(handle: Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize utilities");

    // Clear the screen
    system_table.stdout().reset(false);

    info!("Running on UEFI revision {:?}", system_table.uefi_revision());

    loop {}

    // Status::SUCCESS
}
