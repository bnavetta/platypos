#![no_std]
#![no_main]
#![feature(negative_impls)]

use core::fmt::Write;

use console::Console;

use crate::arch::display::Display;
use crate::arch::interrupts;

mod arch;

mod console;
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
}

/// The shared kernel entry point.
pub fn kmain(args: BootArgs) -> ! {
    log::info!(foo = 1; "Hello, world!");

    let display = args.display.unwrap();
    let mut console = Console::new(display);
    console.clear().unwrap();

    console.write("Hello!\n").unwrap();

    let _ = writeln!(
        &mut console,
        "Hello from PlatypOS v{}",
        env!("CARGO_PKG_VERSION")
    );

    // for _ in 0..1000 {
    //     console.write("text ").unwrap();
    // }

    loop {
        interrupts::halt_until_interrupted();
    }
}
