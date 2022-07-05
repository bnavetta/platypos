//! Kernel defmt logging implementation

use defmt::Encoder;
use spin::Once;

use crate::arch::SerialPort;
use crate::sync::{InterruptSafeMutex, InterruptSafeMutexGuard};

static STATE: Once<InterruptSafeMutex<LogState>> = Once::INIT;

/// Bridge between defmt::Logger's aquire/release API and our mutex
/// See https://github.com/embassy-rs/critical-section/blob/v0.2.7/src/lib.rs#L156-L184
static mut GLOBAL_GUARD: Option<InterruptSafeMutexGuard<LogState>> = None;

/// Marker written to indicate to the host that defmt logging has started. The
/// bootloader can also write to the serial port, so we have to detect that.
const START_OF_OUTPUT: [u8; 4] = [255, 0, 255, 0];

pub fn init(mut serial: SerialPort) {
    STATE.call_once(|| {
        // use core::fmt::Write;
        // let _ = writeln!(&mut serial, "<logging init>");

        write_to(&mut serial, &START_OF_OUTPUT);

        InterruptSafeMutex::new(LogState {
            port: serial,
            encoder: Encoder::new(),
        })
    });
}

struct LogState {
    port: SerialPort,
    encoder: Encoder,
}

#[defmt::global_logger]
struct GlobalKernelLogger;

unsafe impl defmt::Logger for GlobalKernelLogger {
    fn acquire() {
        // TODO: check for reentrance

        let mut guard = STATE.get().expect("Logger not initialized").lock();
        let state = &mut *guard; // Get the inner struct so we can split borrows - https://github.com/rust-lang/rust/issues/72297

        // use core::fmt::Write;
        // let _ = writeln!(&mut state.port, "<acquire>");

        state.encoder.start_frame(|bytes| {
            write_to(&mut state.port, bytes);
        });

        // Safety: at this point, we've locked the mutex, so no one else should touch
        // GLOBAL_GUARD
        let prev = unsafe { GLOBAL_GUARD.replace(guard) };
        debug_assert!(prev.is_none(), "acquire() called twice without a release()");
    }

    unsafe fn flush() {
        // Nothing to do here
    }

    unsafe fn release() {
        // Safety: acquire() must have been called on this thread previously, so we have
        // the lock and can touch GLOBAL_GUARD
        let guard = GLOBAL_GUARD.take();

        match guard {
            Some(mut guard) => {
                let state = &mut *guard;

                // use core::fmt::Write;
                // let _ = writeln!(&mut state.port, "<release>");

                state
                    .encoder
                    .end_frame(|bytes| write_to(&mut state.port, bytes))
            }
            None => panic!("called release() without a call to acquire()"),
        }
    }

    unsafe fn write(bytes: &[u8]) {
        // Safety: this must be called within an acquire/release pair, so the
        // lock is held
        if let Some(ref mut guard) = GLOBAL_GUARD {
            let state = &mut **guard;

            // use core::fmt::Write;
            // let _ = writeln!(&mut state.port, "<write {}>", bytes.len());

            state.encoder.write(bytes, |data| {
                write_to(&mut state.port, data);
            })
        } else {
            panic!("called write() without calling acquire() first");
        }
    }
}

fn write_to(port: &mut SerialPort, bytes: &[u8]) {
    for byte in bytes {
        // TODO: this is an x86_64-specific API
        port.send_raw(*byte);
    }
}

/*

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

*/
