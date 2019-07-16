#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(platypos_test::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[cfg(not(test))]
use core::panic::PanicInfo;

use log::info;

use platypos_pal::Platform;

// Pull in the appropriate platform implementation
#[cfg(target_arch = "x86_64")]
extern crate platypos_platform_x86_64 as platform;

/// Run the PlatypOS kernel. This must be called by an entry point after it has performed any
/// necessary PAL setup.
pub fn run() -> ! {
    info!("Welcome to PlatypOS {}!", env!("CARGO_PKG_VERSION"));
    platform::Platform::halt();
}

#[cfg(not(test))]
#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    use log::error;
    error!("{}", info);
    platform::Platform::halt()
}


#[cfg(test)]
mod tests {
    #[platypos_test::kernel_test]
    fn test_in_kernel() {
        assert_eq!(1, 1);
    }
}

#[cfg(test)]
mod test_entry {
    use bootloader::{BootInfo, entry_point};

    pub fn test_kernel_main(_boot_info: &'static BootInfo) -> ! {
        platypos_test::launch(crate::test_main)
    }

    entry_point!(test_kernel_main);
}