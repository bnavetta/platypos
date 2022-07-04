#![no_std]
#![no_main]
#![feature(alloc_error_handler)]
#![feature(negative_impls)]
#![feature(int_roundings)]
#![feature(allocator_api)]

extern crate alloc;

use core::fmt::Write;

use arch::mm::MemoryAccess;
use console::Console;

use crate::arch::display::Display;
use crate::arch::interrupts;

mod arch;

mod console;
mod error;
mod logging;
mod mm;
mod panic;
mod prelude;
mod sync;

/// Arguments passed from the platform-specific initialization code to
/// [`kmain`].
pub struct BootArgs {
    /// Display handle, if available
    pub display: Option<Display>,

    /// Accessor for physical memory
    pub memory_access: MemoryAccess,
}

/// The shared kernel entry point.
pub fn kmain(args: BootArgs) -> ! {
    let display = args.display.unwrap();
    let mut console = Console::new(display);
    console.clear().unwrap();

    let _ = writeln!(
        &mut console,
        "Hello from PlatypOS v{}",
        env!("CARGO_PKG_VERSION")
    );

    loop {
        interrupts::halt_until_interrupted();
    }
}
