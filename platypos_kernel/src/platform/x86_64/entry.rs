use bootloader::bootinfo::MemoryRegionType;
use bootloader::{entry_point, BootInfo};
use log::info;

use platypos_config;
use serial_logger;

use super::memory;
use crate::platform::PhysicalAddress;

fn main(boot_info: &'static BootInfo) -> ! {
    serial_logger::init(platypos_config::log_levels()).expect("Could not initialize logging");
    memory::init(boot_info);

    for region in boot_info.memory_map.iter() {
        info!(
            "{:#10x} - {:#10x}: {:?}",
            PhysicalAddress::new(region.range.start_addr() as usize),
            PhysicalAddress::new(region.range.end_addr() as usize),
            region.region_type
        );
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

entry_point!(main);
