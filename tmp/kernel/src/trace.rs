//! Kernel tracing and logging
//!
//! The kernel tracing implementation does not allocate, so it may be called from memory management code.
// TODO: make sure this is also interrupt-safe

use core::{
    fmt::{self, Write},
    panic::PanicInfo,
    sync::atomic::{AtomicU64, Ordering},
};

use ansi_rgb::{self, Foreground};
use arrayvec::ArrayVec;
use lazy_static::lazy_static;
use spinning_top::Spinlock;
use tracing::{Event, Level, Metadata, dispatch::{self, Dispatch}, field::{Visit, Field}, span};
use tracing_core::span::Current;
use x86_64::instructions::{hlt, interrupts};

mod backtrace;
mod logger;

use crate::util::BoundedHashMap;
use self::backtrace::Frame;
use self::logger::Logger;

const MAX_LIVE_SPANS: usize = 100;

type SpanMap = BoundedHashMap<span::Id, SpanData, ahash::RandomState, MAX_LIVE_SPANS>;

pub struct Collector {
    id: AtomicU64,
    // TODO: this needs to actually be per-core
    state: Spinlock<LocalState>,

    /// Per-span metadata
    spans: Spinlock<SpanMap>
}

/// Per-core collector state
struct LocalState {
    stack: SpanStack,
}

/// Per-span metadata
#[derive(Debug)]
struct SpanData {
    /// This span's parent
    parent: Option<span::Id>,
    /// Metadata passed at span creation time
    metadata: &'static Metadata<'static>,
    /// Number of references to this span, so we know when it can be removed from the live set
    reference_count: usize,
}

lazy_static! {
    static ref COLLECTOR: Collector = Collector::new();
}

/// Initializes the kernel logging and tracing system.
pub fn init() {
    Logger::initialize();

    let dispatch = Dispatch::from_static(&*COLLECTOR);
    dispatch::set_global_default(dispatch).expect("global default collector already installed")
}


impl Collector {
    // Creates a new Collector, should only be used with the global COLLECTOR above
    fn new() -> Collector {
        Collector {
            id: AtomicU64::new(1),
            state: Spinlock::new(LocalState {
                stack: SpanStack::new(),
            }),
            spans: Spinlock::new(BoundedHashMap::new()),
        }
    }

    /// Runs a function against the core-local collector state
    #[inline]
    fn with_local<T, F: FnOnce(&mut LocalState) -> T>(&self, f: F) -> T {
        interrupts::without_interrupts(|| {
            let mut state = self.state.lock();
            f(&mut state)
        })
    }

    /// Runs a function with access to a span's data. If the span does not exist, returns None
    #[inline]
    fn with_span<T, F: FnOnce(&mut SpanData) -> T>(&self, id: &span::Id, f: F) -> Option<T> {
        self.with_spans(|spans| {
            spans.get_mut(id).map(f)
        })
    }

    /// Runs a function with the global span storage locked
    #[inline]
    fn with_spans<T, F: FnOnce(&mut SpanMap) -> T>(&self, f: F) -> T {
        interrupts::without_interrupts(|| {
            let mut spans = self.spans.lock();
            f(&mut spans)
        })
    }

    /// Gets the current span's ID, if there is one
    fn current_span_id(&self) -> Option<span::Id> {
        self.with_local(|state| state.stack.current())
    }
}

impl tracing::Collect for Collector {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        // TODO: filter by level
        true
    }

    fn new_span(&self, span: &span::Attributes) -> span::Id {
        let next_id = self.id.fetch_add(1, Ordering::SeqCst);
        let parent = if span.is_root() {
            None
        } else if span.is_contextual() {
            self.current_span_id().map(|id| self.clone_span(&id))
        } else {
            span.parent().map(|id| self.clone_span(id))
        };

        let data = SpanData {
            parent,
            metadata: span.metadata(),
            reference_count: 1,
        };

        let id = span::Id::from_u64(next_id);

        self.with_spans(|spans| {
            let prev = spans.insert(id.clone(), data)
                .expect("Exceeded max number of live spans supported");
            assert!(prev.is_none(), "Found existing span {:?} for id {:?}", prev, id);
        });
        
        id
    }

    fn clone_span(&self, id: &span::Id) -> span::Id {
        self.with_span(id, |data| {
            data.reference_count += 1;
        });
        id.clone()
    }

    fn try_close(&self, id: span::Id) -> bool {
        self.with_spans(|spans| {
            let remaining_references = match spans.get_mut(&id) {
                None => 1, // Will avoid condition below
                Some(data) => {
                    data.reference_count -= 1;
                    data.reference_count
                }
            };

            if remaining_references == 0 {
                spans.remove(&id);
                true
            } else {
                false
            }
        })
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {
        // TODO
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
        // TODO
    }

    fn event(&self, event: &Event<'_>) {
        Logger::with(|logger| {
            let metadata = event.metadata();
            logger.emit(metadata.level(), metadata.target(), |w| {
                // Inspired by the tracing_subscriber::fmt Pretty format, log the event fields followed by line and span information

                // The Pretty format puts the whole event field line in the level color, that might work well here too
                struct LogVisitor<'a>(&'a mut logger::LogWriter);

                impl <'a> Visit for LogVisitor<'a> {
                    fn record_debug(&mut self, field: &Field, value: &dyn fmt::Debug) {
                        // Color the field names so it's easier to see where one field ends and the next begins
                        let _ = write!(self.0, " {}: {:?}", field.name().fg(ansi_rgb::magenta()), value);
                    }
                }

                event.record(&mut LogVisitor(w));

                writeln!(w, "\n    {} {}:{}", "at".fg(ansi_rgb::cyan_blue()), metadata.file().unwrap_or("<unknown>"), metadata.line().unwrap_or(0))?;

                // Events either (a) have an explicit parent (b) are contextual children of the current span or (c) are roots
                let id = event.parent().cloned().or_else(|| if event.is_contextual() { self.current_span_id() } else { None });
                if let Some(id) = id {
                    write!(w, "    {} ", "in".fg(ansi_rgb::cyan_blue()))?;
                    self.with_spans(|spans| {
                        print_span_chain(w, spans, &id)
                    })?;
                    writeln!(w)?;
                }

                Ok(())
            });
        })
    }

    fn enter(&self, span: &span::Id) {
        self.with_local(|state| state.stack.push(span.clone()))
    }

    fn exit(&self, span: &span::Id) {
        self.with_local(|state| {
            state.stack.pop(span);
        })
    }

    fn current_span(&self) -> Current {
        match self.current_span_id() {
            Some(span) => {
                let metadata = self.with_span(&span, |s| s.metadata).expect("Current span is missing!");
                Current::new(span, metadata)
            },
            None => Current::none()
        }
    }
}

/// Helper to print out a chain of spans, in the format `grandparent > parent > child`.
fn print_span_chain(writer: &mut impl fmt::Write, spans: &SpanMap, id: &span::Id) -> fmt::Result {
    let span = match spans.get(id) {
        Some(span) => span,
        None => return Ok(())
    };

    if let Some(parent) = span.parent.as_ref() {
        print_span_chain(writer, spans, parent)?;
        write!(writer, " > ")?;
    }

    write!(writer, "{}", span.metadata.name())
}

/// Stack of currently-executing spans. This stack has a fixed depth
struct SpanStack {
    stack: ArrayVec<span::Id, 32>,
}

impl SpanStack {
    pub const fn new() -> SpanStack {
        SpanStack {
            stack: ArrayVec::new_const(),
        }
    }

    // TODO: does this need to handle duplicates like the tracing-subscriber Registry?

    pub fn push(&mut self, id: span::Id) {
        self.stack.try_push(id).expect("Span stack overflow");
    }

    pub fn pop(&mut self, id: &span::Id) {
        let entry = self
            .stack
            .iter()
            .enumerate()
            .rev()
            .find(|(_, current_id)| *current_id == id);
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
    let frame = Frame::current();
    Logger::with(|logger| {
        logger.emit(&Level::ERROR, "PANIC", |w| {
            writeln!(w, "{}", info)?;
            // This is safe-ish, because we know we just grabbed the current frame.
            // We make sure to log the error message before trying this, in case the stack is corrupted.
            unsafe { add_backtrace(w, frame) }
        });
    });
    loop {
        hlt();
    }
}

#[alloc_error_handler]
fn handle_alloc_error(layout: ::core::alloc::Layout) -> ! {
    let frame = Frame::current();
    Logger::with(|logger| {
        logger.emit(&Level::ERROR, "OOM", |w| {
            writeln!(w, "memory allocation of {} bytes failed", layout.size())?;
            // This is safe-ish, because we know we just grabbed the current frame.
            // We make sure to log the error message before trying this, in case the stack is corrupted.
            unsafe { add_backtrace(w, frame) }
        })
    });
    loop {
        hlt();
    }
}

unsafe fn add_backtrace<W: Write>(writer: &mut W, mut frame: Frame) -> fmt::Result {
    // Limit how many stack frames we can walk, in case they're corrupted
    for _ in 0..50 {
        writeln!(writer, "  -> {:#x}", frame.instruction_pointer.as_u64())?;
        match frame.parent() {
            Some(parent) => frame = parent,
            None => break,
        }
    }
    Ok(())
}