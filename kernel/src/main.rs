#![no_std]
#![no_main]
#![feature(asm)]

use core::fmt::Write;
use core::panic::PanicInfo;

use tracing::{info, span, Level};
use x86_64::instructions::{hlt, interrupts};

mod trace;

static foo: &str = "Hello, World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    wait_for_debugger();
    trace::Collector::install();

    let span = span!(Level::INFO, "kernel main");
    let _enter = span.enter();
    info!(foo = 1, "This is traced!");

    panic!("oops");

    loop {
        hlt();
    }
}

/// The GDB setup script will set this to 1 after it's attached
static mut KERNEL_DEBUGGER_ATTACHED: u8 = 0;

#[cfg(feature = "gdb")]
fn wait_for_debugger() {
    unsafe {
        while KERNEL_DEBUGGER_ATTACHED == 0 {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(not(feature = "gdb"))]
fn wait_for_debugger() {}
