#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use slog::{o, info};
use slog_kernel;
use x86_64_ext::instructions::hlt_loop;

mod panic;
mod alloc;

#[export_name = "_start"]
extern "C" fn start() {
    let logger = slog_kernel::kernel_logger(o!("version" => env!("CARGO_PKG_VERSION")));

    info!(logger, "Welcome to PlatypOS!");
    hlt_loop();
}
