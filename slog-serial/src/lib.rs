#![no_std]
use core::fmt::Write;

use slog::{Drain, Record, OwnedKVList};
use spin::Mutex;
use uart_16550::SerialPort;

pub enum SerialError {
    IoError
}

impl From<core::fmt::Error> for SerialError {
    fn from(_: core::fmt::Error) -> SerialError {
        SerialError::IoError
    }
}

pub struct SerialDrain {
    port: Mutex<SerialPort>,
}

impl SerialDrain {
    pub fn new(port: SerialPort) -> SerialDrain {
        SerialDrain {
            port: Mutex::new(port),
        }
    }

    pub unsafe fn on_port(port: u16) -> SerialDrain {
        let mut port = SerialPort::new(port);
        port.init();
        SerialDrain::new(port)
    }
}

impl Drain for SerialDrain {
    type Ok = ();
    type Err = SerialError;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<(), SerialError> {
        let mut w = self.port.lock();

        write!(w, "{} {}:{} {}", record.level(), record.file(), record.line(), record.msg())?;

        // TODO: colors and K/V pairs

        Ok(())
    }
}