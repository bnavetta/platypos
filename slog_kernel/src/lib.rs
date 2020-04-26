//! Kernel logging with slog
#![no_std]

use slog::{Logger, OwnedKV, SendSyncRefUnwindSafeKV};

pub mod serial;
pub mod spinlock;
pub mod fuse_loop;

pub type KernelLogger = Logger<fuse_loop::FuseLoop<spinlock::SpinlockDrain<serial::SerialDrain>>>;

/// Convenience function to create a root kernel logger
pub fn kernel_logger<T>(values: OwnedKV<T>) -> KernelLogger where T: SendSyncRefUnwindSafeKV + 'static {
    // Use the standard serial port
    let drain = unsafe { serial::SerialDrain::at_base(0x3F8) };
    let drain = spinlock::SpinlockDrain::new(drain);
    let drain = fuse_loop::FuseLoop::new(drain);
    Logger::root_typed(drain, values)
}