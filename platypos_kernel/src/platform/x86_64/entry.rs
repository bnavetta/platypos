use bootloader::{BootInfo, entry_point};

use platypos_config;
use serial_logger;

use super::memory;

fn main(boot_info: &'static BootInfo) -> ! {
    serial_logger::init(platypos_config::log_levels()).expect("Could not initialize logging");
    memory::init(boot_info);

    if cfg!(test) {
        #[cfg(test)]
            crate::test_main();
        loop {}
    } else {
        crate::run()
    }
}

entry_point!(main);
