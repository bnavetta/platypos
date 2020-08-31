#![no_std]

use log::info;

use platypos_pal::Platform;

/// Main kernel entry point. This is called after any platform-specific initialization.
pub fn kernel_main<P: Platform>() {
    info!("PlatypOS v{}", env!("CARGO_PKG_VERSION"))
}