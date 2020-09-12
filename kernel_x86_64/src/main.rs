#![no_std]
#![no_main]

// Needed even in 2018 edition since this crate isn't directly referenced
extern crate rlibc;

use core::panic::PanicInfo;
use log::{error, warn, debug};

use platypos_kernel::kernel_main;
use x86_64_ext::instructions::hlt_loop;

mod logger;
mod mem;
mod platform;

use crate::logger::KernelLog;
use crate::platform::Platform;

static LOG: KernelLog = KernelLog::new();

#[export_name = "_start"]
extern "C" fn start() {
    if LOG.init().is_err() {
        // Can't even panic, since that logs
        hlt_loop()
    }
    debug!("Logging initialized");

    kernel_main::<Platform>();

    // We don't expect to get here generally, but loop to be safe
    warn!("Returned from kernel_main");
    hlt_loop()
}

#[panic_handler]
fn handle_panic(info: &PanicInfo) -> ! {
    error!("Kernel panic! {}", info);
    hlt_loop();
}