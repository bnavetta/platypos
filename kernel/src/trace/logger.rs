//! Serial port logger to report tracing information

use core::fmt::{self, Write};

use ansi_rgb::{Foreground, WithForeground};
use spinning_top::Spinlock;
use tracing::Level;
use uart_16550::SerialPort;
use x86_64::instructions::interrupts;

const SERIAL_PORT_BASE: u16 = 0x3F8;

/// Logger backed by the UART 16550 serial port.
pub struct Logger {
    port: SerialPort,
}

static LOGGER: Spinlock<Logger> = Spinlock::new(Logger {
    port: unsafe { SerialPort::new(SERIAL_PORT_BASE) },
});

// Provides some indirection for `Logger::emit`
pub type LogWriter = SerialPort;


impl Logger {

    pub fn with<T, F: FnOnce(&mut Logger) -> T>(f: F) -> T {
        interrupts::without_interrupts(|| {
            let mut logger = LOGGER.lock();
            f(&mut logger)
        })
    }

    /// Performs runtime logger initialization
    pub fn initialize() {
        Logger::with(|logger| logger.port.init())
    }

    pub fn emit<F>(&mut self, level: &Level, target: &str, f: F) where F: FnOnce(&mut LogWriter) -> fmt::Result {
        let _ = write!(&mut self.port, "{} [{}]", level_color(level), target);
        let _ = f(&mut self.port);
    }
}

fn level_color(level: &Level) -> WithForeground<&Level> {
    use ansi_rgb::{blue_magenta, cyan_blue, green, red, yellow};
    let color = match level {
        &Level::ERROR => red(),
        &Level::WARN => yellow(),
        &Level::INFO => cyan_blue(),
        &Level::DEBUG => green(),
        &Level::TRACE => blue_magenta(),
    };
    level.fg(color)
}
