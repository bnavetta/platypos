use core::alloc::Layout;
use core::panic::PanicInfo;

use mini_backtrace::Backtrace;

use crate::arch::interrupts;

const BACKTRACE_DEPTH: usize = 16;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    let bt = Backtrace::<BACKTRACE_DEPTH>::capture();

    defmt::println!(
        "KERNEL PANIC: {}{}",
        defmt::Display2Format(info),
        BacktraceFormat(bt)
    );

    loop {
        interrupts::halt_until_interrupted()
    }
}

struct BacktraceFormat(Backtrace<BACKTRACE_DEPTH>);

impl defmt::Format for BacktraceFormat {
    fn format(&self, fmt: defmt::Formatter) {
        for frame in self.0.frames.iter() {
            defmt::write!(fmt, "  called by {=usize:address}\n", frame);
        }

        if self.0.frames_omitted {
            defmt::write!(fmt, "  ... <frames omitted>\n");
        }
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size());
}
