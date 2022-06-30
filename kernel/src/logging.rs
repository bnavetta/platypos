//! Kernel logging implementation

use core::fmt::Write;

use ansi_rgb::Foreground;
use log::Log;
use spin::Once;

use crate::arch::SerialPort;
use crate::sync::InterruptSafeMutex;

static LOG: Once<KernelLog> = Once::INIT;

pub struct KernelLog {
    inner: InterruptSafeMutex<SerialPort>,
}

/// Initialize the logging system.
pub fn init(serial: SerialPort) {
    log::set_logger(LOG.call_once(|| KernelLog::new(serial))).expect("logger already initialized!");
    log::set_max_level(log::LevelFilter::Trace);
}

// Warning: The logger _must not_ panic, as it's used to print panic messages

impl KernelLog {
    pub const fn new(serial: SerialPort) -> Self {
        KernelLog {
            inner: InterruptSafeMutex::new(serial),
        }
    }
}

impl Log for KernelLog {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        // TODO: configurable logging
        true
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let level_color = match record.level() {
            log::Level::Error => ansi_rgb::red(),
            log::Level::Warn => ansi_rgb::yellow(),
            log::Level::Info => ansi_rgb::green(),
            log::Level::Debug => ansi_rgb::cyan(),
            log::Level::Trace => ansi_rgb::magenta(),
        };

        let mut inner = self.inner.lock();
        let _ = write!(
            &mut inner,
            "{}{} {}",
            record.target().fg(level_color),
            ":".fg(level_color),
            record.args()
        );

        let _ = writeln!(&mut inner);
    }

    fn flush(&self) {
        // no-op
    }
}
