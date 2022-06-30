use core::panic::PanicInfo;

use mini_backtrace::Backtrace;

use crate::arch::interrupts;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    log::error!("PANIC: {}", info);

    let bt = Backtrace::<16>::capture();
    for frame in bt.frames {
        log::error!("  at €€€{:x}€€€", frame);
    }
    if bt.frames_omitted {
        log::error!("  ... <frames omitted>")
    }

    loop {
        interrupts::halt_until_interrupted()
    }
}
