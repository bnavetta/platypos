#![no_std]
#![no_main]
#![feature(asm)]

use core::fmt::Write;
use core::panic::PanicInfo;

use uart_16550::SerialPort;
use x86_64::instructions::{hlt, interrupts};

static foo: &str = "Hello, World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    wait_for_debugger();
    interrupts::disable();

    let mut port = unsafe { SerialPort::new(0x3F8) };
    port.init();

    let _ = writeln!(&mut port, "{}", foo);

    loop {
        hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
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