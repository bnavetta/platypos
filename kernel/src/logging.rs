//! Kernel logging implementation

use core::fmt::Write;

use log::kv::Visitor;
use log::Log;
use spin::Once;

use crate::arch::SerialPort;
use crate::sync::InterruptSafeMutex;

static LOG: Once<KernelLog> = Once::INIT;

pub struct KernelLog {
    inner: InterruptSafeMutex<SerialPort>,
}

/// Initialize lthe ogging system.
pub fn init(serial: SerialPort) {
    log::set_logger(LOG.call_once(|| KernelLog::new(serial))).expect("logger already initialized!");
    log::set_max_level(log::LevelFilter::Trace);
}

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

        let mut inner = self.inner.lock();
        let _ = write!(
            &mut inner,
            "{} {}: {}",
            record.level(),
            record.target(),
            record.args()
        );

        let kvs = record.key_values();
        if kvs.count() > 0 {
            struct FormatVisitor<'a> {
                serial: &'a mut SerialPort,
                first: bool,
            }

            impl<'a, 'kvs> Visitor<'kvs> for FormatVisitor<'a> {
                fn visit_pair(
                    &mut self,
                    key: log::kv::Key<'kvs>,
                    value: log::kv::Value<'kvs>,
                ) -> Result<(), log::kv::Error> {
                    if !self.first {
                        write!(self.serial, " ")?;
                    }

                    write!(self.serial, "{} = {}", key, value)?;
                    Ok(())
                }
            }

            let mut visitor: FormatVisitor<'_> = FormatVisitor {
                serial: &mut *inner,
                first: false,
            };
            let _ = kvs.visit(&mut visitor);
        }
        let _ = writeln!(&mut inner);
    }

    fn flush(&self) {
        // no-op
    }
}
