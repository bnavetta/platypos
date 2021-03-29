//! Serial port logger to report tracing information

use core::{alloc::Layout, fmt::{self, Write}};
use core::panic::PanicInfo;

use ansi_rgb::{Foreground, WithForeground};
use spinning_top::Spinlock;
use tracing::{
    field::{Field, Visit},
    Event, Level,
};
use uart_16550::SerialPort;
use x86_64::instructions::interrupts;

const SERIAL_PORT_BASE: u16 = 0x3F8;

use super::backtrace::Frame;

/// Logger backed by the UART 16550 serial port.
pub struct Logger {
    port: SerialPort,
}

static LOGGER: Spinlock<Logger> = Spinlock::new(Logger {
    port: unsafe { SerialPort::new(SERIAL_PORT_BASE) },
});

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

    pub fn log_event(&mut self, event: &Event) {
        let metadata = event.metadata();
        let _ = write!(
            &mut self.port,
            "{} [{}] -",
            level_color(metadata.level()),
            metadata.target()
        );

        let mut visitor = SerialVisitor {
            port: &mut self.port,
        };
        event.record(&mut visitor);

        let _ = writeln!(&mut self.port);
    }

    // TODO: better abstraction for these

    pub fn log_panic(&mut self, panic: &PanicInfo) {
        use ansi_rgb::red;
        let _ = writeln!(&mut self.port, "{}: {}", "PANIC".fg(red()), panic);
    }

    pub fn log_allocation_failure(&mut self, layout: Layout) {
        use ansi_rgb::{red, cyan_blue};
        let _ = writeln!(&mut self.port, "{}: memory allocation of {} bytes failed", "OOM".fg(red()), layout.size().fg(cyan_blue()));
    }

    /// Logs a backtrace starting at `frame`
    pub unsafe fn log_backtrace(&mut self, mut frame: Frame) {
        // Limit how many stack frames we can walk, in case they're corrupted
        for _ in 0..50 {
            let _ = writeln!(&mut self.port, "  -> {:#x}", frame.instruction_pointer.as_u64());
            match frame.parent() {
                Some(parent) => frame = parent,
                None => break,
            }
        }
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

/// Visitor for tracing values that writes them to the serial port
struct SerialVisitor<'a> {
    port: &'a mut SerialPort,
}

impl<'a> Visit for SerialVisitor<'a> {
    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
        // Color the field names so it's easier to see where one field ends and the next begins
        use ansi_rgb::magenta;
        let _ = write!(self.port, " {}: {:?}", field.name().fg(magenta()), value);
    }
}
