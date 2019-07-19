#![no_std]

use core::panic::PanicInfo;

use log::{error, info};

use platypos_config;
use serial_logger;

// reexport the test macro
pub use platypos_test_macro::kernel_test;

mod qemu;

pub trait TestCase {
    fn name(&self) -> &'static str;
    fn run(&self);
}

pub fn test_runner(tests: &[&dyn TestCase]) {
    info!("Found {} tests", tests.len());

    for test in tests {
        info!("Running {}", test.name());
        test.run();
    }

    info!("All tests passed!");
    qemu::exit(qemu::ExitCode::Success);
}

#[panic_handler]
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    error!("Test failed: {}", info);

    qemu::exit(qemu::ExitCode::Failure);
}

/// Helper for implementing the kernel entry point when running tests. The entry point cannot be
/// entirely implemented in platypos_test because it requires access to the generated test harness
/// main function.
pub fn launch(test_main: fn() -> ()) -> ! {
    serial_logger::init(platypos_config::log_levels()).expect("Could not initialize logging");

    test_main();

    loop {}
}
