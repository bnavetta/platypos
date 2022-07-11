#![no_std]
#![no_main]
#![allow(unstable_name_collisions)]
#![feature(alloc_error_handler)]
#![feature(allocator_api)]
#![feature(const_maybe_uninit_uninit_array)]
#![feature(int_roundings)]
#![feature(maybe_uninit_uninit_array)]
#![feature(negative_impls)]

extern crate alloc;
extern crate ktest;

use core::fmt::Write;

use arch::mm::MemoryAccess;
use console::Console;

use crate::arch::display::Display;
use crate::arch::interrupts;

mod arch;

mod console;
mod error;
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
    let _span = tracing::info_span!("kmain", at = kmain as usize).entered();

    #[cfg(test)]
    ktest::run_tests();

    let display = args.display.unwrap();
    let mut console = Console::new(display);
    console.clear().unwrap();

    let _ = writeln!(
        &mut console,
        "Hello from PlatypOS v{}",
        env!("CARGO_PKG_VERSION")
    );

    let mut s = ::alloc::string::String::new();
    s.push_str("Hello, World!");
    tracing::trace!("Heap-allocated string: {}", s);
    drop(s);

    test_inline();

    loop {
        interrupts::halt_until_interrupted();
    }
}

#[inline(always)]
fn test_inline() {
    panic!("This is inline");
}
