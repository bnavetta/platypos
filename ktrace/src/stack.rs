//! Span stack for tracking the current span on a CPU core.

use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicUsize, Ordering};

use tracing_core::span;

/// Maximum depth of the per-core span stack.
const MAX_DEPTH: usize = 32;

/// Span entry stack. This is interrupt-safe on a single processor, but cannot
/// be shared across processors.
pub struct SpanStack {
    /// Current end of the stack. This uses an atomic integer so we can
    /// guarantee correct ordering in the face of interrupts, even if it's
    /// only single-core.
    end: AtomicUsize,
    slots: [MaybeUninit<span::Id>; MAX_DEPTH],
}

// Just enable/disable interrupts around stack access - even with atomics, can't
// safely manipulate stack for example: .push() is called, bumps the index, then
// immediately interrupted and interrupt handler calls .pop
// alternatively, don't track current span!

impl SpanStack {
    pub const fn new() -> Self {
        Self {
            end: AtomicUsize::new(0),
            slots: MaybeUninit::uninit_array(),
        }
    }

    /// Push a new span onto the end of the stack, making it the new
    /// [`current()`] span. If the stack is full, this returns `false` instead
    /// of adding the span.
    pub fn push(&mut self, id: span::Id) -> bool {
        let idx = self.end.fetch_add(1, Ordering::AcqRel);
        if idx == MAX_DEPTH {
            false
        } else {
            self.slots[idx].write(id);
            true
        }
    }

    /// Get the current span from the stack.
    pub fn current(&self) -> Option<span::Id> {
        // TODO: probably do need to disable interrupts here - otherwise, an interrupt
        // could exit a span in between reading `end` and accessing `slots`. In
        // practice, this is likely fine since interrupt handlers will typically create
        // and then close their own spans, with a net-zero effect on `end`.
        let end = *self.end.get_mut();
        if end != 0 {
            // Safety: if the end is nonzero, then the stack is non-empty and we can access
            // the last element
            Some(unsafe { self.slots[end - 1].assume_init_ref() }.clone())
        } else {
            None
        }
    }
}

impl Default for SpanStack {
    fn default() -> Self {
        Self::new()
    }
}
