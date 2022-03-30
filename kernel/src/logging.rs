//! Kernel logging implementation

use core::fmt::Write;

use log::kv::Visitor;
use log::Log;
use platypos_platform::Platform;

use crate::sync::InterruptSafeMutex;

pub struct KernelLog<P: Platform> {
    inner: InterruptSafeMutex<P, P::Serial>,
}

impl<P: Platform> KernelLog<P> {
    pub const fn new(serial: P::Serial) -> Self {
        KernelLog {
            inner: InterruptSafeMutex::new(serial),
        }
    }
}

impl<P: Platform> Log for KernelLog<P> {
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
            struct FormatVisitor<'a, P: Platform> {
                serial: &'a mut P::Serial,
                first: bool,
            }

            impl<'a, 'kvs, P: Platform> Visitor<'kvs> for FormatVisitor<'a, P> {
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

            let mut visitor: FormatVisitor<'_, P> = FormatVisitor {
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

// This is an interesting dilemma: we can't create global variables in the core
// kernel crate that are parameterized by a Platform type, because globals can't
// be generic. This means that all global state has to be instantiated in
// platform-specific crates, which is possibly OK because it limits truly-global
// variables. If that's not workable, probably have to go with conditional
// compilation instead of generics.
