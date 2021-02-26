//! Slog drain that provides concurrency safety with a spinlock.

use slog::{Drain, OwnedKVList, Record};
use spinning_top::Spinlock;
use x86_64::instructions::interrupts::without_interrupts;

/// Drain that wraps another drain with a spinlock. Safe to use from interrupt handlers.
pub struct SpinlockDrain<D: Drain> {
    inner: Spinlock<D>,
}

impl<D: Drain> SpinlockDrain<D> {
    pub fn new(drain: D) -> SpinlockDrain<D> {
        SpinlockDrain {
            inner: Spinlock::new(drain),
        }
    }
}

impl<D: Drain> Drain for SpinlockDrain<D> {
    type Ok = D::Ok;
    type Err = D::Err;

    fn log(&self, record: &Record, values: &OwnedKVList) -> Result<D::Ok, D::Err> {
        without_interrupts(|| {
            let inner = self.inner.lock();
            inner.log(record, values)
        })
    }
}
