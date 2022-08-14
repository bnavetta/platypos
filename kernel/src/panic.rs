use core::alloc::Layout;
use core::panic::PanicInfo;

use mini_backtrace::Backtrace;

const BACKTRACE_DEPTH: usize = 16;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let span = tracing::error_span!("panic").entered();

    let bt = Backtrace::<BACKTRACE_DEPTH>::capture();

    tracing::error!("{}", info);

    for frame in bt.frames.iter() {
        tracing::error!(at = *frame, "backtrace");
    }

    if bt.frames_omitted {
        tracing::error!("... <frames omitted>");
    }

    span.exit(); // Close the span before spin-looping
    crate::arch::hal_impl::fatal_error();
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size());
}
