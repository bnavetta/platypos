//! Kernel tracing and logging
//!
//! The kernel tracing implementation does not allocate, so it may be called from memory management code.
// TODO: make sure this is also interrupt-safe

use core::{panic::PanicInfo, sync::atomic::{AtomicU64, Ordering}};

use arrayvec::ArrayVec;
use spinning_top::Spinlock;
use tracing::{span, Event, Metadata, dispatch::{self, Dispatch}};
use tracing_core::span::Current;
use x86_64::instructions::{interrupts, hlt};

mod backtrace;
mod logger;

use self::backtrace::Frame;
use self::logger::Logger;

pub struct Collector {
    id: AtomicU64,
    // TODO: this needs to actually be per-core
    state: Spinlock<LocalState>,
}


/// Per-core collector state
struct LocalState {
    stack: SpanStack,
}

static COLLECTOR: Collector = Collector {
    id: AtomicU64::new(1),
    state: Spinlock::new(LocalState {
        stack: SpanStack::new()
    }),
};

impl Collector {
    /// Installs the kernel Collector as the global default
    pub fn install() {
        Logger::initialize();

        let dispatch = Dispatch::from_static(&COLLECTOR);
        dispatch::set_global_default(dispatch).expect("global default collector already installed")
    }

    /// Runs a function against the core-local collector state
    #[inline]
    fn with_local<T, F: FnOnce(&mut LocalState) -> T>(&self, f: F) -> T {
        interrupts::without_interrupts(|| {
            let mut state = self.state.lock();
            f(&mut state)
        })
    }
}

impl tracing::Collect for Collector {
    fn enabled(&self, metadata: &Metadata<'_>) -> bool {
        // TODO: filter by level
        true
    }

    fn new_span(&self, span: &span::Attributes) -> span::Id {
        let next_id = self.id.fetch_add(1, Ordering::SeqCst);
        // TODO: deal with attributes
        // TODO: keep track of open spans?
        span::Id::from_u64(next_id)
    }

    fn record(&self, span: &span::Id, values: &span::Record<'_>) {
        // TODO
    }

    fn record_follows_from(&self, span: &span::Id, follows: &span::Id) {
        // TODO

    }

    fn event(&self, event: &Event<'_>) {
        Logger::with(|logger| logger.log_event(event))
    }

    fn enter(&self, span: &span::Id) {
        self.with_local(|state| {
            state.stack.push(span.clone())
        })
    }

    fn exit(&self, span: &span::Id) {
        self.with_local(|state| {
            state.stack.pop(span);
        })
    }

    // fn current_span(&self) -> Current {
    //     self.with_local(|state| match state.stack.current() {
    //         Some(span) => Current::new(span),
    //         None => Current::none()
    //     })
    // }
}

/// Stack of currently-executing spans. This stack has a fixed depth
struct SpanStack {
    stack: ArrayVec<[span::Id; 32]>
}

impl SpanStack {
    pub const fn new() -> SpanStack {
        SpanStack {
            stack: ArrayVec::new()
        }
    }

    // TODO: does this need to handle duplicates like the tracing-subscriber Registry?

    pub fn push(&mut self, id: span::Id) {
        self.stack.try_push(id).expect("Span stack overflow");
    }

    pub fn pop(&mut self, id: &span::Id) {
        let entry = self.stack.iter().enumerate().rev().find(|(_, current_id)| *current_id == id);
        if let Some((index, _)) = entry {
            self.stack.remove(index);
        }
    }

    pub fn current(&self) -> Option<span::Id> {
        self.stack.last().cloned()
    }
}

#[panic_handler]
fn handle_panic(info: &PanicInfo) -> ! {
    let mut frame = Frame::current();
    Logger::with(|logger| {
        logger.log_panic(info);

        // This is safe-ish, because we know we just grabbed the current frame.
        // We make sure to log the panic message before trying this, in case the stack is corrupted.
        for _ in 0..50 {
            logger.log_backtrace_frame(&frame);
            match unsafe { frame.parent() } {
                Some(parent) => frame = parent,
                None => break,
            }
        }
    });
    loop {
        hlt();
    }
}