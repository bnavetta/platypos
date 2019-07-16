#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]

use core::panic::PanicInfo;

use log::{info, error};

// reexport the test macro
pub use platypos_test_macro::kernel_test;

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
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    error!("{}", info);

    loop {}
}

