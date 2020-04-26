use core::alloc::Layout;
use core::panic::PanicInfo;

use slog::crit;

use crate::root_logger;

#[cfg(target_arch = "x86_64")]
#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    crit!(root_logger(), "Kernel panic: {}", info);
    x86_64_ext::instructions::hlt_loop();
}

// This is here rather than in the alloc module since I think of it as a special case of the panic handler

#[alloc_error_handler]
fn handle_alloc_error(layout: Layout) -> ! {
    panic!("Allocating {:?} failed", layout);
}