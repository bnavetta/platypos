#![no_std]
#![no_main]

use core::fmt::Write;

use uart_16550::SerialPort;

mod panic;

#[export_name = "_start"]
extern "C" fn start() {
    let mut serial_port = unsafe { SerialPort::new(0x3F8) };;
    serial_port.init();
    let _ = writeln!(&mut serial_port, "Welcome to PlatypOS!");
    loop {}
}
