mod entry;

pub mod display;
pub mod interrupts;

/// Type of the serial port [`::core::fmt::Write`] implementation
pub type SerialPort = uart_16550::SerialPort;
