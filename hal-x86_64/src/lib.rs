#![no_std]

use core::convert::Infallible;

pub mod interrupts;

/// UART 16550 serial port writer
pub struct SerialPort(uart_16550::SerialPort);

impl SerialPort {
    /// Create and initialize a serial port driver
    ///
    /// # Safety
    /// The caller must ensure that the given port address points to a valid
    /// serial port device. Otherwise, this may write to an arbitrary I/O port.
    pub unsafe fn new(port: u16) -> Self {
        let mut inner = uart_16550::SerialPort::new(port);
        inner.init();
        Self(inner)
    }
}

impl platypos_hal::Write for SerialPort {
    type Error = Infallible;

    fn write_all(&mut self, data: &[u8]) -> Result<(), Self::Error> {
        for byte in data {
            // DO NOT use `.send` - it encodes the values 8 and 0x7F specially, which causes
            // a whole bunch of problems using it with binary postcard data.
            self.0.send_raw(*byte)
        }

        Ok(())
    }

    fn flush(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Called by the kernel after panic handling completes.
pub fn fatal_error() -> ! {
    // This function is only ever called _from_ the panic handler, so it must not
    // panic
    loop {
        x86_64::instructions::interrupts::enable_and_hlt()
    }
}
