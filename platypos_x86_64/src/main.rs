#![no_std]
#![no_main]

use bootloader::{BootInfo, entry_point};

use platypos_config;
use platypos_kernel;
use serial_logger;

fn main(boot_info: &'static BootInfo) -> ! {
    serial_logger::init(platypos_config::log_levels()).expect("Could not initialize logging");

    if cfg!(test) {
        #[cfg(test)]
        test_main();
        loop {}
    } else {
        platypos_kernel::run()
    }
}

entry_point!(main);
