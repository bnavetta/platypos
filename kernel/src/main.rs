#![no_std]
#![no_main]
#![feature(alloc_error_handler)]

use conquer_once::spin::OnceCell;

use slog::{Logger, o, info};
use slog_kernel::fuse_loop::FuseLoop;
use slog_kernel::serial::SerialDrain;
use slog_kernel::spinlock::SpinlockDrain;

use x86_64::instructions::interrupts::without_interrupts;
use x86_64_ext::instructions::hlt_loop;

use platypos_boot_info::BootInfo;

mod panic;
mod alloc;

pub type KernelDrain = FuseLoop<SpinlockDrain<SerialDrain>>;
pub type KernelLogger = Logger<&'static KernelDrain>;

/// Global OnceCell to hold the slog drain, which allows using Logger::new without wrapping the drain in Arc
static ROOT_DRAIN: OnceCell<KernelDrain> = OnceCell::uninit();

/// Global OnceCell holding the root logger, so it's accessible to panic and interrupt handlers
static ROOT_LOGGER: OnceCell<KernelLogger> = OnceCell::uninit();

///Get a reference to the root logger. If the kernel logger has not been initialized, this will do so.
pub fn root_logger() -> &'static KernelLogger {
    without_interrupts(|| {
        ROOT_LOGGER.get_or_init(|| {
            let drain = ROOT_DRAIN.get_or_init(|| {
                // Standard serial port
                let drain = unsafe { SerialDrain::at_base(0x3F8) };
                let drain = SpinlockDrain::new(drain);
                FuseLoop::new(drain)
            });

            Logger::root_typed(drain, o!("version" => env!("CARGO_PKG_VERSION")))
        })
    })
}

#[export_name = "_start"]
extern "C" fn start(boot_info: &'static BootInfo) {
    // Important: we want to initialize logging here, as soon as possible. The first call to root_logger initializes the kernel logger,
    // and we want to avoid doing that in an interrupt handler
    let logger = root_logger();

    info!(logger, "Welcome to PlatypOS!");
    info!(logger, "Boot info:\n{}", boot_info);
    hlt_loop();
}
