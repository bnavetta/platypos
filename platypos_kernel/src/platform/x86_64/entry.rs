use log::info;

use platypos_config;
use serial_logger;

use super::memory;
use crate::platform::PhysicalAddress;

use platypos_boot_info::BootInfo;

#[export_name = "_start"]
extern "C" fn start(boot_info: &'static BootInfo) -> ! {
    serial_logger::init(platypos_config::log_levels()).expect("Could not initialize logging");

    info!("Hello, World!");


    for entry in boot_info.memory_map().iter() {
        info!("- {:?}", entry);
    }

    #[cfg(test)] {
        crate::test_main();
    }

    super::halt();

    //    super::init_perprocessor_data();
    //
    //    if cfg!(test) {
    //        #[cfg(test)]
    //            crate::test_main();
    //        loop {}
    //    } else {
    //        crate::run()
    //    }
}

#[cfg(test)]
#[platypos_test::kernel_test]
fn fail() {
    assert!(false);
}