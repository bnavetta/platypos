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

use platypos_hal::interrupts::Controller;

use arch::mm::MemoryAccess;
use console::Console;
use mm::root_allocator::Allocator;

use crate::arch::display::Display;

mod arch;

mod console;
mod error;
mod mm;
mod panic;
mod prelude;

/// Arguments passed from the platform-specific initialization code to
/// [`kmain`].
pub struct BootArgs {
    /// Display handle, if available
    pub display: Option<Display>,

    /// Accessor for physical memory
    pub memory_access: &'static MemoryAccess,

    /// Root memory allocator
    pub root_allocator: &'static Allocator<'static>,

    pub interrupt_controller: &'static arch::hal_impl::interrupts::Controller,

    pub trace_worker: platypos_ktrace::Worker<arch::hal_impl::SerialPort>,
}

/// The shared kernel entry point.
pub fn kmain(mut args: BootArgs) -> ! {
    let _span = tracing::info_span!("kmain", at = kmain as usize).entered();
    args.trace_worker.work(); // Manually driving until there's a task executor
                              // TODO: make sure panic handler can flush worker

    #[cfg(test)]
    {
        ktest::run_tests();
        args.trace_worker.work(); // TODO: run during tests too
    }

    let display = args.display.unwrap();
    let mut console = Console::new(display);
    console.clear().unwrap();

    let _ = writeln!(
        &mut console,
        "Hello from PlatypOS v{}",
        env!("CARGO_PKG_VERSION")
    );

    loop {
        args.interrupt_controller.wait();
    }
}

#[inline(always)]
fn test_inline() {
    panic!("This is inline");
}

fn test_alloc(allocator: &Allocator) {
    let small_allocation = allocator.allocate(4).unwrap();
    let big_allocation = allocator.allocate(1000).unwrap();
    tracing::info!(
        "Small allocation: {}\nBig allocation: {}",
        small_allocation,
        big_allocation
    );

    allocator.dump_state();

    allocator.deallocate(big_allocation).unwrap();
    assert!(
        allocator.deallocate(big_allocation).is_err(),
        "Allowed double-free"
    );

    allocator.dump_state();
}
