//! Serial port logger to report tracing information

use core::fmt::{self, Write};
use core::panic::PanicInfo;

use ansi_rgb::{Foreground, WithForeground};
use tracing::{Event, Level, field::{Value, Visit, Field}};
use spinning_top::Spinlock;
use uart_16550::SerialPort;
use x86_64::instructions::interrupts;

const SERIAL_PORT: u16 = 0x3F8;

/// Logger backed by the UART 16550 serial port.
pub struct Logger {
    port: Spinlock<SerialPort>,
}

impl Logger {
    /// Creates a new Logger. [`init`] must be called before the logger is used.
    pub const fn new() -> Logger {
        Logger {
            port: Spinlock::new(unsafe { SerialPort::new(SERIAL_PORT) })
        }
    }

    /// Performs runtime logger initialization
    pub fn init(&self) {
        let mut port = self.port.lock();
        port.init();
    }

    pub fn log_event(&self, event: &Event) {
        interrupts::without_interrupts(|| {
            let mut port = self.port.lock();
            let metadata = event.metadata();
            let _ = write!(&mut port, "{} [{}] -", level_color(metadata.level()), metadata.target());

            let mut visitor = SerialVisitor { port: &mut port };
            event.record(&mut visitor);

            let _ = writeln!(&mut port);
        })
    }

    pub fn log_panic(&self, panic: &PanicInfo) {
        use ansi_rgb::red;
        interrupts::without_interrupts(|| {
            let mut port = self.port.lock();
            let _ = writeln!(&mut port, "{}: {}", "PANIC".fg(red()), panic);
        })
    }
}

fn level_color(level: &Level) -> WithForeground<&Level> {
    use ansi_rgb::{red, yellow, green, cyan_blue, blue_magenta};
    let color = match level {
        &Level::ERROR => red(),
        &Level::WARN => yellow(),
        &Level::INFO => cyan_blue(),
        &Level::DEBUG => green(),
        &Level::TRACE => blue_magenta()
    };
    level.fg(color)
}

/// Visitor for tracing values that writes them to the serial port
struct SerialVisitor<'a> {
    port: &'a mut SerialPort
}

impl <'a> Visit for SerialVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // Color the field names so it's easier to see where one field ends and the next begins
        use ansi_rgb::magenta;
        let _ = write!(self.port, " {}: {:?}", field.name().fg(magenta()), value);
    }
}