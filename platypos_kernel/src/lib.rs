#![no_std]

use core::panic::PanicInfo;

use log::{error, info};

use platypos_pal::Platform;

#[cfg(any(test, testing))]
use platypos_test::test_panic_handler;

// Pull in the appropriate platform implementation
#[cfg(target_arch = "x86_64")]
extern crate platypos_platform_x86_64 as platform;

/// Run the PlatypOS kernel. This must be called by an entry point after it has performed any
/// necessary PAL setup.
pub fn run() -> ! {
    info!("Welcome to PlatypOS {}!", env!("CARGO_PKG_VERSION"));
    platform::Platform::halt();
}

#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    #[cfg(any(test, testing))]
    platypos_test::test_panic_handler(info);

    error!("{}", info);
    platform::Platform::halt();
}