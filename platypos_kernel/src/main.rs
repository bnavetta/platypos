#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(platypos_test::test_runner)]
#![reexport_test_harness_main = "test_main"]

#[cfg(not(test))]
use core::panic::PanicInfo;

use log::info;

use platypos_config;

// Pull in the appropriate platform implementation
#[cfg_attr(target_arch = "x86_64", path = "platform/x86_64/mod.rs")]
#[allow(unused_attributes)]
#[path = "platform/x86_64/mod.rs"] // Default for IDE completion
mod platform;

/// Run the PlatypOS kernel. This must be called by the platform-specific entry point after
/// performing any necessary setup
pub fn run() -> ! {
    info!("Welcome to PlatypOS {} ({})!", env!("CARGO_PKG_VERSION"), platypos_config::build_revision());

    platform::halt();
}

#[cfg(not(test))]
#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    use log::error;
    error!("{}", info);
    platform::halt()
}

#[cfg(test)]
mod tests {
    #[platypos_test::kernel_test]
    fn test_pass() {
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