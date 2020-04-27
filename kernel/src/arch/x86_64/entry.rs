use slog::info;

use platypos_boot_info::BootInfo;

use crate::{kernel_main, root_logger};

#[export_name = "_start"]
extern "C" fn start(boot_info: &'static BootInfo) -> ! {
    // Important: we want to initialize logging here, as soon as possible. The first call to root_logger initializes the kernel logger,
    // and we want to avoid doing that in an interrupt handler
    let logger = root_logger();

    info!(logger, "Boot info:\n{}", boot_info);

    kernel_main()
}