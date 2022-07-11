#![no_std]

use core::convert::Infallible;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicBool, Ordering};

use ciborium_io::Write;
use platypos_ktrace_proto as proto;
use spin::Mutex;
use tracing_core::{span, Dispatch, Subscriber};

pub struct KTrace<W> {
    // TOOD: this _really_ needs to be an InterruptSafeMutex, or sadness will ensue
    // TODO: use RWLock for inner? More generally, finer-grained locking
    inner: Mutex<Inner<W>>,
}

/// Maximum depth of the "current span" call stack
const MAX_DEPTH: usize = 16;

/// Maximum number of active spans allowed
const MAX_ACTIVE_SPANS: usize = 128;

// TODO: separate out per-core state (span stack, whether or not in tracing
// code)

/// Used to track if `ktrace` code is currently running, so that code it might
/// call (particularly memory allocators) can avoid recursive trace calls.
static IN_TRACING: AtomicBool = AtomicBool::new(false);

struct Inner<W> {
    next_id: NonZeroU64,
    writer: W,
    // TODO: we can probably afford dynamic allocation here, as long as we handle the
    // logging-from-tracing-code case
    span_stack: heapless::Vec<span::Id, MAX_DEPTH>,
    active_spans: heapless::FnvIndexMap<SpanId, SpanState, MAX_ACTIVE_SPANS>,
}

#[derive(Debug)]
struct SpanState {
    references: usize,
    metadata: &'static tracing_core::Metadata<'static>,
}

/// Newtype wrapper for implementing [`hash32::Hash`] on [`span::Id`]. Remove
/// once [`heapless`] updates to [`hash32`] 0.3
#[derive(PartialEq, Eq, Debug)]
struct SpanId(span::Id);

/// Initialize `ktrace` as the `tracing` subscriber.
pub fn init<W: Write<Error = Infallible> + Send + 'static>(mut writer: W) {
    writer
        .write_all(&proto::START_OF_OUTPUT)
        .expect("Could not write start-of-output");
    let dispatch = Dispatch::new(KTrace::new(writer));
    tracing_core::dispatcher::set_global_default(dispatch).expect("Tracing initialized twice");
}

/// Tests if this was called from inside the `ktrace` implementation
pub fn is_tracing() -> bool {
    IN_TRACING.load(Ordering::Acquire)
}

/// Evaluates an expression only if _not_ inside the `ktrace` implementation,
/// wrapping it in an [`Option`].
#[macro_export]
macro_rules! if_not_tracing {
    ($e:expr) => {
        if $crate::is_tracing() {
            None
        } else {
            Some($e)
        }
    };
}

impl<W: Write<Error = Infallible> + Send> KTrace<W> {
    fn new(writer: W) -> Self {
        Self {
            inner: Mutex::new(Inner {
                next_id: NonZeroU64::new(1).unwrap(),
                writer,
                span_stack: heapless::Vec::new(),
                active_spans: heapless::FnvIndexMap::new(),
            }),
        }
    }
}

/// Execute `f` with the [`IN_TRACING`] flag set. This must be used for any
/// non-trivial [`Subscriber`] methods that may call out to other kernel
/// subsystems. Otherwise, they might call `ktrace` while `ktrace` is calling
/// them!
#[inline(always)]
fn in_tracing<T, F: FnOnce() -> T>(f: F) -> T {
    IN_TRACING.store(true, Ordering::Release);
    let result = f();
    IN_TRACING.store(false, Ordering::Release);
    result
}

impl<W: Write<Error = Infallible> + Send + 'static> Subscriber for KTrace<W> {
    fn enabled(&self, _metadata: &tracing_core::Metadata<'_>) -> bool {
        true
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        in_tracing(|| {
            let mut inner = self.inner.lock();
            let id = inner.next_id;
            inner.next_id = inner.next_id.checked_add(1).expect("span ID overflow");

            let parent = if span.is_root() {
                None
            } else if span.is_contextual() {
                inner.current().cloned()
            } else {
                span.parent().cloned()
            };

            let id = span::Id::from_non_zero_u64(id);

            let state = SpanState {
                metadata: span.metadata(),
                references: 1,
            };
            inner
                .active_spans
                .insert(SpanId(id.clone()), state)
                .expect("too many spans");

            inner.emit(&proto::Message::SpanCreated(proto::SpanCreated {
                id: id.into_u64(),
                parent: parent.map(|s| s.into_u64()),
                metadata: proto::Metadata::from_tracing(span.metadata()),
                fields: span.into(),
            }));

            id
        })
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {
        todo!()
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
        todo!()
    }

    fn event(&self, event: &tracing_core::Event<'_>) {
        in_tracing(|| {
            let mut inner = self.inner.lock();
            let span_id = if event.is_contextual() {
                inner.current()
            } else if event.is_root() {
                None
            } else {
                event.parent()
            }
            .map(|i| i.into_u64());

            inner.emit(&proto::Message::Event(proto::Event {
                span_id,
                metadata: proto::Metadata::from_tracing(event.metadata()),
                fields: event.into(),
            }));
        })
    }

    fn enter(&self, span: &span::Id) {
        let mut inner = self.inner.lock();
        inner.push_span(span.clone());
    }

    fn exit(&self, span: &span::Id) {
        in_tracing(|| {
            // TODO: handle duplicates and out-of-order exiting?
            let mut inner = self.inner.lock();
            let popped = inner.pop_span();
            assert!(popped == Some(span.clone()), "Popped non-current span");
        })
    }

    fn max_level_hint(&self) -> Option<tracing_core::LevelFilter> {
        None
    }

    fn event_enabled(&self, event: &tracing_core::Event<'_>) -> bool {
        let _ = event;
        true
    }

    fn clone_span(&self, id: &span::Id) -> span::Id {
        in_tracing(|| {
            let mut inner = self.inner.lock();
            let state = inner
                .span_state_mut(id)
                .expect("Cloning a span with no state");
            state.references += 1;
            id.clone()
        })
    }

    fn try_close(&self, id: span::Id) -> bool {
        in_tracing(|| {
            let mut inner = self.inner.lock();
            let state = inner
                .span_state_mut(&id)
                .expect("Cloning a span with no state");
            state.references -= 1;

            if state.references == 0 {
                inner.active_spans.remove(&SpanId(id.clone()));

                inner.emit(&proto::Message::SpanClosed { id: id.into_u64() });

                true
            } else {
                false
            }
        })
    }

    fn current_span(&self) -> span::Current {
        in_tracing(|| {
            let inner = self.inner.lock();
            if let Some(id) = inner.current() {
                let state = inner.span_state(id).expect("current span has no state");
                span::Current::new(id.clone(), state.metadata)
            } else {
                span::Current::none()
            }
        })
    }
}

impl<W: Write<Error = Infallible> + Send> Inner<W> {
    fn span_state(&self, id: &span::Id) -> Option<&SpanState> {
        self.active_spans.get(&SpanId(id.clone()))
    }

    fn span_state_mut(&mut self, id: &span::Id) -> Option<&mut SpanState> {
        self.active_spans.get_mut(&SpanId(id.clone()))
    }

    fn emit(&mut self, message: &proto::SenderMessage) {
        let storage = StreamOut(&mut self.writer);
        postcard::serialize_with_flavor(message, storage).expect("Sending failed");

        // TODO: COBS needs to modify data after it's written. Can probably get
        // streaming working  by just writing the message, and having the host
        // read more if it gets an unexpected EOF error from postcard
        // ALSO: look at collect_str method when serializing, can use Display
        // impl
    }

    fn current(&self) -> Option<&span::Id> {
        self.span_stack.last()
    }

    fn push_span(&mut self, id: span::Id) {
        self.span_stack.push(id).expect("Span stack depth exceeded");
    }

    fn pop_span(&mut self) -> Option<span::Id> {
        self.span_stack.pop()
    }
}

struct StreamOut<'a, W: Write>(&'a mut W);

impl<'a, W: Write> postcard::ser_flavors::Flavor for StreamOut<'a, W> {
    type Output = ();

    fn try_push(&mut self, data: u8) -> postcard::Result<()> {
        self.0
            .write_all(&[data])
            .map_err(|_| postcard::Error::SerdeSerCustom)
    }

    fn try_extend(&mut self, data: &[u8]) -> postcard::Result<()> {
        self.0
            .write_all(data)
            .map_err(|_| postcard::Error::SerdeSerCustom)
    }

    fn finalize(self) -> postcard::Result<Self::Output> {
        self.0.flush().map_err(|_| postcard::Error::SerdeSerCustom)
    }
}

impl hash32::Hash for SpanId {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash32::Hasher,
    {
        self.0.into_u64().hash(state)
    }
}