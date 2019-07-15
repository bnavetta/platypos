#![no_std]
#![no_main]

use core::panic::PanicInfo;

use bootloader::{BootInfo, entry_point};
use log::{info, error};

use config::Config;
use serial_logger;

mod platform;
mod util;

fn main(boot_info: &'static BootInfo) -> ! {
    serial_logger::init(Config::log_settings()).expect("Could not initialize logging");

    // TODO: platform method for physical->virtual address mapping

    // Startup sequence:
    // 1. Platform code enables logging
    // 2. Platform calls kernel_allocators::init(cb), where cb is a callback the
    //    platform can use to add regions to the physical memory allocator
    // 3. Platform initializes rest of hardware (i.e. GDT, TSS, IDT, processor topology)
    // 4. Platform calls kernel_core::run(platform)
    // 5. kernel_core can call back into platform to enumerate and start other processors,
    //    specifying the entry point to call on each

    info!("Welcome to PlatypOS!");

    loop {}
}

entry_point!(main);

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);

    if cfg!(test) {
        util::qemu::exit(util::qemu::ExitCode::Failure);
    } else {
        util::hlt_loop();
    }

}