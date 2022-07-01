use core::alloc::Layout;
use core::panic::PanicInfo;

use ansi_rgb::{red, Foreground};
use mini_backtrace::Backtrace;

use crate::arch::interrupts;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("{} {info}", "KERNEL PANIC:".fg(red()));

    let bt = Backtrace::<16>::capture();
    for frame in bt.frames {
        // The wrapper tool knows to look for this format and symbolize it
        log::error!("  called by €€€{:x}€€€", frame);
    }
    if bt.frames_omitted {
        log::error!("  ... <frames omitted>")
    }

    loop {
        interrupts::halt_until_interrupted()
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size());
}
