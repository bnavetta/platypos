use core::convert::Infallible;

mod entry;

pub mod display;
pub mod interrupts;
pub mod mm;

/// Type of the serial port [`::core::fmt::Write`] implementation
pub struct SerialPort(uart_16550::SerialPort);

/// The base page size for this platform.
pub const PAGE_SIZE: usize = 4096;

impl SerialPort {
    /// Create and initialize a serial port driver
    pub unsafe fn new(port: u16) -> Self {
        let mut inner = uart_16550::SerialPort::new(port);
        inner.init();
        Self(inner)
    }
}

impl ciborium_io::Write for SerialPort {
    type Error = Infallible;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        for byte in data {
            self.0.send(*byte)
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
