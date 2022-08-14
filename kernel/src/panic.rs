use core::alloc::Layout;
use core::panic::PanicInfo;

use mini_backtrace::Backtrace;
use platypos_common::sync::Global;

const BACKTRACE_DEPTH: usize = 16;

static ABORT: Global<fn() -> !> = Global::new();

/// Set the global "abort" handler. This is called by the panic implementation
/// to stop the panicking processor.
///
/// # Safety
/// The caller must provide a function that does not panic, as it will be called
/// from within the panic implementation.
///
/// # Panics
/// If an abort handler has already been set.
pub(crate) unsafe fn set_abort_handler(handler: fn() -> !) {
    ABORT.init(handler);
}

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
    match ABORT.try_get() {
        Some(abort) => loop {
            abort()
        },
        None => loop {
            ::core::hint::spin_loop();
        },
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size());
}
