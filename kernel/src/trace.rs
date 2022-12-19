//! Kernel tracing setup. This ties [`platypos_ktrace`] into the rest of the
//! kernel.
//!
//! In particular, it manages the background I/O task, with an emphasis on being
//! able to get traces during a panic.

use platypos_common::sync::Global;
use platypos_ktrace::Worker;

use crate::arch::hal_impl::SerialPort;
use crate::prelude::InterruptSafeMutex;

static WORKER: Global<InterruptSafeMutex<'static, Worker<SerialPort>>> = Global::new();

/// Initialize kernel tracing
pub(crate) fn init(
    writer: SerialPort,
    topology: &'static crate::arch::hal_impl::topology::Topology,
    controller: &'static crate::arch::hal_impl::interrupts::Controller,
) {
    let worker = platypos_ktrace::init(writer, topology);
    WORKER.init(InterruptSafeMutex::new(controller, worker));
}

/// Try to flush any pending trace events.
pub(crate) fn flush() {
    if let Some(mut worker) = WORKER.try_get().and_then(|m| m.try_lock()) {
        worker.work();
    }
    // Silently ignore if:
    // - another core is already running the worker (we don't care _which_ core
    //   does the I/O)
    // - tracing hasn't been initialized yet
}

// Once we have a scheduler, it'll start a task which holds the spinlock and
// runs the worker
