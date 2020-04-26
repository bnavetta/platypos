//! Drain for logging to the 16550 UART serial port

use core::cell::{RefCell, BorrowMutError};
use core::fmt::{self, Write, Arguments};

use slog::{Drain, Record, Level, KV, Key, OwnedKVList, Serializer};

use uart_16550::SerialPort;

/// Slog `Drain` that writes to a [16550 UART serial port](https://en.wikipedia.org/wiki/16550_UART). It is not safe to concurrently log to
/// this drain, and it must be protected by additional synchronization.
pub struct SerialDrain {
    port: RefCell<SerialPort>,
}

impl SerialDrain {
    /// Creates a new `SerialDrain` writing to the given port. The serial port must
    /// already be initialized.
    pub fn new(port: SerialPort) -> SerialDrain {
        SerialDrain { port: RefCell::new(port) }
    }

    /// Creates a new `SerialDrain` writing to the port at the given base address. Unsafe because the caller
    /// must ensure that `base` refers to a serial I/O port.
    pub unsafe fn at_base(base: u16) -> SerialDrain {
        let mut port = SerialPort::new(base);
        port.init();
        SerialDrain::new(port)
    }
}

// ANSI color codes
const COLOR_RESET: &str = "\x1b[0m";
const COLOR_RED: &str = "\x1b[31m";
const COLOR_GREEN: &str = "\x1b[32m";
const COLOR_YELLOW: &str = "\x1b[33m";
const COLOR_BLUE: &str = "\x1b[34m";
const COLOR_CYAN: &str = "\x1b[36m";
const COLOR_WHITE: &str = "\x1b[37m";
const COLOR_GREY: &str = "\x1b[90m";
const COLOR_BRIGHT_RED: &str = "\x1b[91m";

impl Drain for SerialDrain {
    type Ok = ();
    type Err = SerialDrainError;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<(), SerialDrainError> {
        let mut port = self.port.try_borrow_mut()?;

        let level_color = match record.level() {
            Level::Trace    => COLOR_WHITE,
            Level::Debug    => COLOR_BLUE,
            Level::Info     => COLOR_GREEN,
            Level::Warning  => COLOR_YELLOW,
            Level::Error    => COLOR_RED,
            Level::Critical => COLOR_BRIGHT_RED
        };
        write!(port, "{}{}{} ", COLOR_RESET, level_color, record.level().as_str())?;
        write!(port, "{}[{}:{}]", COLOR_CYAN, record.file(), record.line())?;
        write!(port, "{} - {}{}", COLOR_GREY, COLOR_RESET, record.msg())?;

        let mut serializer = SerialPortSerializer { port: &mut port };
        record.kv().serialize(record, &mut serializer)?;
        values.serialize(record, &mut serializer)?;
        writeln!(port)?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum SerialDrainError {
    /// There were multiple concurrent attempts to log to this drain
    ConcurrentLogError,

    /// There was an error writing to the serial port
    WriteError(fmt::Error),

    /// Slog produced an error
    SlogError(slog::Error)
}

impl From<fmt::Error> for SerialDrainError {
    fn from(err: fmt::Error) -> SerialDrainError {
        SerialDrainError::WriteError(err)
    }
}

impl From<BorrowMutError> for SerialDrainError {
    fn from(_err: BorrowMutError) -> SerialDrainError {
        SerialDrainError::ConcurrentLogError
    }
}

impl From<slog::Error> for SerialDrainError {
    fn from(err: slog::Error) -> SerialDrainError {
        SerialDrainError::SlogError(err)
    }
}

/// Implementation of slog's `Serializer` trait that writes K/V pairs to a serial port
struct SerialPortSerializer<'a> {
    port: &'a mut SerialPort
}

impl <'a> Serializer for SerialPortSerializer<'a> {
    fn emit_arguments(&mut self, key: Key, val: &Arguments) -> slog::Result {
        write!(self.port, " {} = {}", key, val)?;
        Ok(())
    }

    // TODO: implement other methods?
}