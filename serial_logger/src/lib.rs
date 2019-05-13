#![no_std]

use core::fmt::Write;

use log::{Level, LevelFilter, Log, Metadata, Record, SetLoggerError};
use spin::Mutex;
use uart_16550::SerialPort;
use x86_64::instructions::interrupts::without_interrupts;

const PORT: u16 = 0x3F8;

const COLOR_GREY: &'static str = "\x1b[90m";
const COLOR_WHITE: &'static str = "\x1b[37m";
const COLOR_BLUE: &'static str = "\x1b[34m";
const COLOR_GREEN: &'static str = "\x1b[32m";
const COLOR_YELLOW: &'static str = "\x1b[33m";
const COLOR_RED: &'static str = "\x1b[31m";
const COLOR_BRIGHT_CYAN: &'static str = "\x1b[96m";
const COLOR_NORMAL: &'static str = "\x1b[0m";

static LOGGER: SerialLogger = unsafe { SerialLogger::from_port(PORT) };

pub fn init() -> Result<(), SetLoggerError> {
    LOGGER.init();
    log::set_logger(&LOGGER)?;

    if cfg!(debug_assertions) {
        log::set_max_level(LevelFilter::Trace);
    } else {
        log::set_max_level(LevelFilter::Info);
    }

    Ok(())
}

pub struct SerialLogger {
    port: Mutex<SerialPort>,
}

impl SerialLogger {
    const fn new(port: SerialPort) -> SerialLogger {
        SerialLogger {
            port: Mutex::new(port),
        }
    }

    const unsafe fn from_port(port: u16) -> SerialLogger {
        let serial_port = SerialPort::new(port);
        SerialLogger::new(serial_port)
    }

    fn init(&self) {
        let mut port = self.port.lock();
        port.init();
    }
}

impl Log for SerialLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            without_interrupts(|| { // make sure this can be used in interrupt handlers
                let mut w = self.port.lock();

                let level_color = match record.level() {
                    Level::Trace => COLOR_WHITE,
                    Level::Debug => COLOR_BLUE,
                    Level::Info => COLOR_GREEN,
                    Level::Warn => COLOR_YELLOW,
                    Level::Error => COLOR_RED,
                };

                let _ = write!(
                    w,
                    "{}[{:<30}]{} ",
                    COLOR_GREY,
                    record.module_path().unwrap_or(record.target()),
                    COLOR_NORMAL
                );
                let _ = write!(w, "{}{:>5}{} ", level_color, record.level(), COLOR_NORMAL);
                let _ = write!(
                    w,
                    "{}{}:{}{}",
                    COLOR_BRIGHT_CYAN,
                    record.file().unwrap_or("unknown"),
                    record.line().unwrap_or(0),
                    COLOR_NORMAL
                );
                let _ = write!(w, " {}-{} {}\n", COLOR_GREY, COLOR_NORMAL, record.args());
            })
        }
    }

    fn flush(&self) {}
}
