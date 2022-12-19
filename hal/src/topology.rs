//! System topology, mostly for handling multiple processors/cores.

use alloc::boxed::Box;
use alloc::vec::Vec;

/// A processor identifier. Regardless of the underlying platform convention,
/// these are expected to be consecutive values starting from 0, suitable for
/// array indices.
pub type ProcessorId = u16;

pub trait Topology: Send + Sync {
    /// The maximum number of processors supported on this platform.
    const MAX_PROCESSORS: u16;

    /// Get the ID of the processor this function is called from.
    ///
    /// # Implementation Note
    /// This is a highly performance-sensitive function, since it's used to
    /// implement per-processor variables. Wherever possible, inlining and the
    /// use of platform-specific fast processor identification (e.g. via special
    /// registers) is encouraged.
    fn current_processor(&self) -> ProcessorId;
}

impl<T: Topology> Topology for &'static T {
    const MAX_PROCESSORS: u16 = T::MAX_PROCESSORS;

    fn current_processor(&self) -> ProcessorId {
        <T as Topology>::current_processor(self)
    }
}

#[cfg(loom)]
pub mod loom {
    use core::sync::atomic::{AtomicU16, Ordering};

    /// Topology for Loom concurrency tests
    pub struct LoomTopology;

    pub static TOPOLOGY: LoomTopology = LoomTopology;

    loom::lazy_static! {
        // Use loom::lazy_static so this is reset after each iteration
        static ref NEXT_ID: AtomicU16 = AtomicU16::new(0);
    }

    loom::thread_local! {
        static CURRENT_ID: u16 = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    }

    impl super::Topology for LoomTopology {
        const MAX_PROCESSORS: u16 = loom::MAX_THREADS as u16;

        #[inline(always)]
        fn current_processor(&self) -> super::ProcessorId {
            CURRENT_ID.with(|c| *c)
        }
    }
}

#[cfg(loom)]
mod cell {
    pub use loom::cell::UnsafeCell;
}

#[cfg(not(loom))]
mod cell {
    pub(super) struct UnsafeCell<T>(core::cell::UnsafeCell<T>);

    impl<T> UnsafeCell<T> {
        pub(super) fn new(data: T) -> Self {
            Self(core::cell::UnsafeCell::new(data))
        }

        pub(super) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
            f(self.0.get())
        }
    }
}

// TODO: belongs in its own crate

pub struct PerProcessor<T, TP: Topology> {
    topology: TP,
    // Ideally this would just be an array, but that's blocked on https://github.com/rust-lang/rust/issues/60551
    // One advantage of Box, though, is that it prevents PerProcessor itself from being large
    values: Box<[cell::UnsafeCell<Option<T>>]>,
}

impl<T, TP: Topology> PerProcessor<T, TP> {
    /// Create a new `PerProcessor` with the given CPU topology.
    ///
    /// This will heap-allocate backing storage based on [`TP::MAX_PROCESSORS`].
    pub fn new(topology: TP) -> Self {
        let mut values = Vec::with_capacity(TP::MAX_PROCESSORS as usize);
        for _ in 0..TP::MAX_PROCESSORS {
            values.push(cell::UnsafeCell::new(None));
        }

        Self {
            topology,
            values: values.into_boxed_slice(),
        }
    }

    /// Get a reference to this processor's cell
    pub fn with_mut<R>(&self, f: impl FnOnce(&mut Option<T>) -> R) -> R {
        let idx = self.topology.current_processor() as usize;
        self.values[idx].with_mut(|ptr| f(unsafe { ptr.as_mut().unwrap() }))
    }
}

// Sync because T is bound to a specific processor and only accessible on that
// processor
unsafe impl<T, TP: Topology> Sync for PerProcessor<T, TP> {}

#[cfg(all(test, loom))]
mod test {
    use super::loom::LoomTopology;
    use super::PerProcessor;

    loom::lazy_static! {
        static ref VAR: PerProcessor<usize, LoomTopology> = PerProcessor::new(LoomTopology);
    }

    #[test]
    fn test_per_processor() {
        loom::model(|| {
            let threads = (0..loom::MAX_THREADS - 1)
                .map(|i| {
                    loom::thread::spawn(move || {
                        VAR.with_mut(|cell| *cell = Some(i));
                        VAR.with_mut(|cell| assert_eq!(*cell, Some(i)));
                    })
                })
                .collect::<Vec<_>>();

            for t in threads.into_iter() {
                t.join().unwrap();
            }
        })
    }
}
