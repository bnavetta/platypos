#![no_std]

use core::fmt::Write;

use console::Console;
use platypos_platform::Platform;

mod console;
mod panic;

/// Arguments passed from the platform-specific initialization code to
/// [`kmain`].
pub struct BootArgs<P: Platform> {
    /// Display handle, if available
    pub display: Option<P::Display>,
}

/// The shared kernel entry point.
pub fn kmain<P: Platform>(args: BootArgs<P>) -> ! {
    let display = args.display.unwrap();
    let mut console: Console<P> = Console::new(display);
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

    loop {}
}
