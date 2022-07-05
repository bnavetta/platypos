use core::alloc::Layout;
use core::panic::PanicInfo;

use mini_backtrace::Backtrace;

use crate::arch::interrupts;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // defmt::println!("{} {}", "KERNEL PANIC:".fg(red()), info);
    defmt::println!("KERNEL PANIC: {}", defmt::Display2Format(info));

    let bt = Backtrace::<16>::capture();
    for frame in bt.frames {
        // The wrapper tool knows to look for this format and symbolize it
        // TODO: custom defmt display hint instead
        defmt::println!("  called by €€€{:x}€€€", frame);
    }
    if bt.frames_omitted {
        defmt::println!("  ... <frames omitted>")
    }

    loop {
        interrupts::halt_until_interrupted()
    }
}

#[alloc_error_handler]
fn alloc_error_handler(layout: Layout) -> ! {
    panic!("memory allocation of {} bytes failed", layout.size());
}
