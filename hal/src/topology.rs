//! System topology, mostly for handling multiple processors/cores.

use alloc::boxed::Box;
use alloc::vec::Vec;

/// A processor identifier. Regardless of the underlying platform convention,
/// these are expected to be consecutive values starting from 0, suitable for
/// array indices.
type ProcessorId = u32;

pub trait Topology {
    /// The maximum number of processors supported on this platform.
    const MAX_PROCESSORS: usize;

    /// Get the ID of the processor this function is called from.
    ///
    /// # Implementation Note
    /// This is a highly performance-sensitive function, since it's used to
    /// implement per-processor variables. Wherever possible, inlining and the
    /// use of platform-specific fast processor identification (e.g. via special
    /// registers) is encouraged.
    fn current_processor(&self) -> ProcessorId;
}

#[cfg(loom)]
pub mod loom {
    use core::sync::atomic::{AtomicU32, Ordering};

    /// Topology for Loom concurrency tests
    pub struct LoomTopology;

    static NEXT_ID: AtomicU32 = AtomicU32::new(0);

    loom::thread_local! {
        static CURRENT_ID: u32 = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    }

    impl super::Topology for LoomTopology {
        const MAX_PROCESSORS: usize = loom::MAX_THREADS;

        #[inline(always)]
        fn current_processor(&self) -> super::ProcessorId {
            CURRENT_ID.with(|c| *c)
        }
    }
}

// TODO: belongs in its own crate

pub struct PerProcessor<T, TP: Topology> {
    topology: TP,
    // Ideally this would just be an array, but that's blocked on https://github.com/rust-lang/rust/issues/60551
    // One advantage of Box, though, is that it prevents PerProcessor itself from being large
    values: Box<[Option<T>]>,
}

impl<T, TP: Topology> PerProcessor<T, TP> {
    /// Create a new `PerProcessor` with the given CPU topology.
    ///
    /// This will heap-allocate backing storage based on [`TP::MAX_PROCESSORS`].
    pub fn new(topology: TP) -> Self {
        let mut values = Vec::with_capacity(TP::MAX_PROCESSORS);
        for _ in 0..TP::MAX_PROCESSORS {
            values.push(None);
        }
        Self {
            topology,
            values: values.into_boxed_slice(),
        }
    }

    /// Get a reference to this processor's cell
    fn get_mut(&self) -> &mut Option<T> {
        let idx = self.topology.current_processor() as usize;
        // self.values.p
        &mut self.values[idx]
    }
}
