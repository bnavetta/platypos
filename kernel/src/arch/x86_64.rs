mod entry;

pub mod display;
pub mod interrupts;
pub mod mm;

/// Type of the serial port [`::core::fmt::Write`] implementation
pub type SerialPort = uart_16550::SerialPort;

/// The base page size for this platform.
pub const PAGE_SIZE: usize = 4096;
