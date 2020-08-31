//! Kernel logger implementation
// https://github.com/tokio-rs/tracing would be really neat to have for os-level instrumentation
// However, it's a lot of wiring to set up, and requires memory allocation (ex. to store span
// metadata), limiting the contexts where it can be used.

use core::fmt::Write;

use ansi_rgb::Foreground;
use log::{Log, Level, Metadata, Record, SetLoggerError, LevelFilter};
use rgb::RGB8;
use spinning_top::Spinlock;
use uart_16550::SerialPort;
use x86_64::instructions::interrupts;
use x86_64_ext::instructions::hlt_loop;

/// Serial port to log in
const SERIAL_PORT: u16 = 0x3f8;

// Colors chosen to match my terminal theme
const TRACE_COLOR: RGB8 = RGB8::new(0xef, 0xb5, 0xf7);
const DEBUG_COLOR: RGB8 = RGB8::new(0x8a, 0xb7, 0xd9);
const INFO_COLOR: RGB8 = RGB8::new(0xc2, 0xe0, 0x75);
const WARN_COLOR: RGB8 = RGB8::new(0xe1, 0xe4, 0x8b);
const ERROR_COLOR: RGB8 = RGB8::new(0xf0, 0x0c, 0x0c);

pub struct KernelLog {
    port: Spinlock<SerialPort>
}

impl KernelLog {
    /// Create a new kernel logger. This is `const` so that the logger can be given a `'static`
    /// lifetime. However, this means the logger is not fully initialized until `init` is called.
    pub const fn new() -> KernelLog {
        // Safety: we know this is the standard serial communication port
        // https://wiki.osdev.org/Serial_Ports
        let port = unsafe { SerialPort::new(SERIAL_PORT) };
        KernelLog {
            port: Spinlock::new(port)
        }
    }

    /// Perform runtime initialization of the kernel logger.
    pub fn init(&'static self) -> Result<(), SetLoggerError> {
        let mut port = self.port.lock();
        port.init();
        log::set_max_level(LevelFilter::Trace);
        log::set_logger(self)
    }
}

impl Log for KernelLog {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        // TODO: filtering
        true
    }

    fn log(&self, record: &Record) {
        let (level_pad, level_color) = match record.level() {
            Level::Trace => ("", TRACE_COLOR),
            Level::Debug => ("", DEBUG_COLOR),
            Level::Info => (" ", INFO_COLOR),
            Level::Warn => (" ", WARN_COLOR),
            Level::Error => ("", ERROR_COLOR)
        };

        // Since we can log from interrupt handlers, avoid deadlocks using the serial port
        interrupts::without_interrupts(|| {
            let mut port = self.port.lock();
            // The fg wrapper doesn't implement width padding, so do it manually
            let _ = writeln!(&mut port, "{}{} {} - {}", level_pad, record.level().fg(level_color), record.target(), record.args());
        });
    }

    fn flush(&self) {
        // Nothing to do
    }
}