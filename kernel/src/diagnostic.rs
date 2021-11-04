//! Kernel diagnostics
//!
//! This module supports both the `tracing` and `log` ecosystems. Tracing
//! provides richer functionality, and so it's preferred in almost all cases.
//! However, particularly low-level parts of the kernel (especially the memory
//! allocator) may be called from within the tracing implementation. To avoid
//! recursing between the tracing collector and those systems, they should use
//! `log` instead.

use core::fmt::Write;
use core::sync::atomic::{AtomicU64, Ordering};
use core::{fmt, mem};

use arrayvec::ArrayString;
use owo_colors::{AnsiColors, OwoColorize};
use tracing::{field, span, Subscriber};

use crate::arch::sync::UninterruptibleSpinlock;
use crate::driver::uart::Uart;

/// Output for kernel diagnostic messages
enum KernelOutput {
    Early,
    Serial { uart: Uart },
}

/// Tracing subscriber/collector implementation
struct KernelCollector {
    next_span_id: AtomicU64,
}

/// Logging adapter
struct KernelLog;

static OUT: UninterruptibleSpinlock<KernelOutput> =
    UninterruptibleSpinlock::new(KernelOutput::Early);

/// Buffer for logging messages before the serial console is initialized.
// This is separate from the `KernelLog` structure to avoid making it
// unnecessarily large.
static EARLY_BUF: UninterruptibleSpinlock<ArrayString<4096>> =
    UninterruptibleSpinlock::new(ArrayString::new_const());

// Since the tracing implementation is allowed to allocate (and the allocator
// isn't allowed to trace), this can allocate for span data That means HashMap,
// etc. should be available (and possibly tracing's own default registry?)
// May want separate early allocators
// - one for globals that won't ever be freed (Arc for tracing collector,
//   tracing callsite registry, data for DeviceTree index)
// - one that will be the regular kernel allocator (supports freeing), which
//   trace data can go in from the start

static COLLECTOR: KernelCollector = KernelCollector {
    next_span_id: AtomicU64::new(1),
};

static LOG: KernelLog = KernelLog;

pub fn init() {
    log::set_logger(&LOG).expect("Could not install logger");
    log::set_max_level(log::LevelFilter::Trace);
    tracing::subscriber::set_global_default(&COLLECTOR)
        .expect("Could not install tracing subscriber");
}

/// Switch diagnostic output to a serial console. This will flush any early-boot
/// messages to the console.
pub fn enable_serial(uart: Uart) {
    let mut log = OUT.lock();
    let prev = mem::replace(&mut *log, KernelOutput::Serial { uart });
    if let KernelOutput::Early = prev {
        // Dump any messages accumulated during early boot
        let _ = log.write_str(EARLY_BUF.lock().as_str());
    } else {
        panic!("Kernel log already in serial mode");
    }
}

pub fn _macro_print(args: fmt::Arguments) {
    let mut log = OUT.lock();
    let _ = log.write_fmt(args);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::diagnostic::_macro_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

impl KernelOutput {
    /// Writes out the header for a diagnostic message. Logging and tracing
    /// implementations use this for consistency.
    ///
    /// # Parameters
    /// - `level`: the log level
    /// - `target`: the system the message originated in
    /// - `file`: the source file the message originated from (if available)
    /// - `line`: the line within `file` (if available)
    #[inline]
    fn diagnostic_header<L: DiagnosticLevel>(
        &mut self,
        level: L,
        target: &str,
        file: Option<&str>,
        line: Option<u32>,
    ) -> fmt::Result {
        write!(
            self,
            "{} {}",
            level.color(level.level_color()),
            target.bold()
        )?;
        match (file, line) {
            (Some(file), Some(line)) => {
                write!(self, " {}", format_args!("({}:{})", file, line).dimmed())?
            }
            (Some(file), None) => write!(self, " {}", format_args!("({})", file).dimmed())?,
            (None, Some(line)) => {
                write!(self, " {}", format_args!("(<unknown>:{})", line).dimmed())?
            }
            (None, None) => (),
        }
        write!(self, ":")?;
        Ok(())
    }
}

impl fmt::Write for KernelOutput {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        match self {
            KernelOutput::Early => EARLY_BUF.lock().try_push_str(s).map_err(|_| fmt::Error),
            KernelOutput::Serial { uart } => uart.write_str(s),
        }
    }
}

impl Subscriber for &'static KernelCollector {
    fn enabled(&self, _metadata: &tracing::Metadata<'_>) -> bool {
        // TODO: filtering
        true
    }

    fn new_span(&self, _span: &span::Attributes<'_>) -> span::Id {
        // Relaxed is fine because we only need atomicity
        span::Id::from_u64(self.next_span_id.fetch_add(1, Ordering::Relaxed))
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {
        // TODO
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
        // TODO
    }

    fn event(&self, event: &tracing::Event<'_>) {
        let mut log = OUT.lock();
        let metadata = event.metadata();

        let _ = log.diagnostic_header(
            metadata.level(),
            metadata.target(),
            metadata.file(),
            metadata.line(),
        );
        event.record(&mut FieldVisitor { log: &mut *log });
        let _ = writeln!(log);
    }

    fn enter(&self, _span: &span::Id) {
        // TODO
    }

    fn exit(&self, _span: &span::Id) {
        // TODO
    }
}

struct FieldVisitor<'a> {
    log: &'a mut KernelOutput,
}

impl<'a> field::Visit for FieldVisitor<'a> {
    fn record_str(&mut self, field: &field::Field, value: &str) {
        let _ = write!(self.log, " {} = {}", field.name(), value);
    }

    fn record_debug(&mut self, field: &field::Field, value: &dyn fmt::Debug) {
        let _ = write!(self.log, " {} = {:?}", field.name(), value);
    }
}

impl log::Log for KernelLog {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        // TODO
        true
    }

    fn log(&self, record: &log::Record) {
        let mut out = OUT.lock();
        let _ = out.diagnostic_header(
            record.level(),
            record.target(),
            record.file(),
            record.line(),
        );
        let _ = writeln!(out, " {}", record.args());
    }

    fn flush(&self) {
        // Nothing to do
    }
}

/// Adapter for log and tracing levels with the requirements for printing them
trait DiagnosticLevel: Copy + fmt::Display {
    fn level_color(self) -> AnsiColors;
}

impl DiagnosticLevel for &tracing::Level {
    fn level_color(self) -> AnsiColors {
        use tracing::Level;
        match *self {
            Level::ERROR => AnsiColors::BrightRed,
            Level::WARN => AnsiColors::Yellow,
            Level::INFO => AnsiColors::Green,
            Level::DEBUG => AnsiColors::Blue,
            Level::TRACE => AnsiColors::White,
        }
    }
}

impl DiagnosticLevel for log::Level {
    fn level_color(self) -> AnsiColors {
        use log::Level;
        match self {
            Level::Error => AnsiColors::BrightRed,
            Level::Warn => AnsiColors::Yellow,
            Level::Info => AnsiColors::Green,
            Level::Debug => AnsiColors::Blue,
            Level::Trace => AnsiColors::White,
        }
    }
}
