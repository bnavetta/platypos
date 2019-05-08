use serial_logger;

use bootloader::{entry_point, BootInfo};
use log::info;

use crate::qemu;
use crate::memory::frame;

fn test_kernel_main(boot_info: &'static BootInfo) -> ! {
    serial_logger::init().expect("Could not initialize logging");

    frame::init(boot_info);

    super::test_main();
    loop {}
}

entry_point!(test_kernel_main);

pub fn test_runner(tests: &[&dyn Fn()]) {
    info!("Running {} tests", tests.len());

    for test in tests {
        test();
    }

    qemu::exit(qemu::ExitCode::Success);
}