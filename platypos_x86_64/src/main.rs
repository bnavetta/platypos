#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![feature(custom_test_frameworks)]
#![test_runner(platypos_test::test_runner)]

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

//#[cfg(test)]
mod tests {
    use platypos_test::kernel_test;

    #[kernel_test]
    fn test_foo() {
        assert_eq!(1, 1);
    }
}