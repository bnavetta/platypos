//! The `dbg` crate is a library for debug logging via a serial port. It's modeled after the
//! Weenix `dbg` macros.
#![no_std]
#![feature(option_replace)]
#![deny(missing_docs)]

extern crate spin;
extern crate uart_16550;

use core::fmt::{Arguments, Result, Write};

use uart_16550::SerialPort;
use spin::Mutex;

pub use crate::category::Category;
use crate::category::COLOR_NORMAL;

mod category;

static PORT: Mutex<Option<SerialPort>> = Mutex::new(None);

/// Initialize the debug logging library on serial port `port`.
pub fn init(port: u16) {
    let mut serial_port = SerialPort::new(port);
    serial_port.init();

    let mut port = PORT.lock();
    if port.is_some() {
        panic!("Already initialized")
    }

    port.replace(serial_port);
}

/// Print a format message to the debug console.
pub fn print(args: Arguments) -> Result {
    PORT.lock().as_mut().expect("dbg library not initialized").write_fmt(args)
}

/// Macro for printing to the debug console.
#[macro_export]
macro_rules! dbg_print {
    ($($arg:tt)*) => {
        $crate::print(format_args!($($arg)*)).expect("Serial communication failed");
    }
}

/// Macro for printing to the debug console.
///
/// Uses the [`format!`] syntax to write data with a newline.
///
/// [`format!`]: https://doc.rust-lang.org/std/macro.format.html
#[macro_export]
macro_rules! dbg_println {
    () => (dbg_print!("\n"));
    ($msg:expr) => (dbg_print!(concat!($msg, "\n")));
    ($fmt:expr, $($arg:tt)*) => (dbg_print!(concat!($fmt, "\n"), $($arg)*));
}

/// Macro for writing a debug message.
///
/// Writes a message using [`format!`] syntax with a particular debug `category`. The category
/// determines if and how to display the message.
///
/// [`format!`]: https://doc.rust-lang.org/std/macro.format.html
#[macro_export]
macro_rules! dbg {
    ($category:expr, $($arg:tt)*) => ($crate::do_debug($category, file!(), line!(), format_args!($($arg)*)));
}

/// Internal function for implementing the [`dbg!`] macro.
///
/// [`dbg`] macro.dbg.html
pub fn do_debug(category: Category, file: &'static str, line: u32, args: Arguments) {
    // TODO:
    dbg_println!("{}[{}] {}:{} : {}{}", category.color(), category.name(), file, line, args, COLOR_NORMAL);
}
