//! Kernel Tracing
//!
//! This is an implementation of [`tracing_core`] APIs for use in the PlatypOS
//! kernel.
//!
//! # Goals
//! * Usable from interrupt and memory-allocator contexts. This implies that
//!   tracing entry points cannot allocate and any locks must be
//!   interrupt-aware.
//! * Sufficiently-structured so that hosts can enhance trace data (for example,
//!   automatically mapping kernel addresses to source code location)
//!
//! # Design
//! `ktrace` introduces a lightweight [schema](https://dl.acm.org/doi/10.1145/3544497.3544500) on top of
//! [`tracing_core`]'s APIs. In particular, attributes of an event or span must
//! be predeclared and of a specific type.
//!
//! Serialized trace data is streamed over a serial (or other I/O) port, and a
//! tool on the other end reconstructs and formats the traces.
//!
//! In-kernel span metadata is stored in a sharded fixed-size slab inspired by
//! [sharded-slab](https://lib.rs/crates/sharded-slab). In addition, I/O is handled by a worker task
//! via [`thingbuf`] so as to not block interrupt handlers and other critical
//! code.
//!
//! This reduces the work done when creating trace data, allowing it to be used
//! during interrupt handling and memory allocation. It also avoids contention
//! between cores when tracing. However, interrupts must still be disabled
//! during modifications of internal tracing data structures, which cannot be
//! updated reentrantly.
#![no_std]
#![feature(maybe_uninit_uninit_array)]

extern crate alloc;

use core::convert::Infallible;
use core::num::NonZeroU64;
use core::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use hashbrown::hash_map::Entry;
use platypos_hal::topology::PerProcessor;
use platypos_hal::Write;
use platypos_ktrace_proto as proto;

use hashbrown::HashMap;
use platypos_slab::Slab;
use serde::Serialize;
// use stack::SpanStack;
use thingbuf::recycling::{self, Recycle};
use thingbuf::StaticThingBuf;
use tracing_core::{span, Dispatch, Subscriber};

// mod stack;

// Maximum number of spans which can exist at once
const MAX_SPANS: usize = 128;

/// Shared kernel tracing subscriber
pub struct KTrace<TP: platypos_hal::topology::Topology + 'static> {
    spans: Slab<MAX_SPANS, SpanState, TP>,
    // stack: PerProcessor<SpanStack, &'static TP>,
}

/// Worker task which sends serialized trace events to the host
pub struct Worker<W: Write> {
    writer: W,
    total_events: usize,
}

/// Per-span state that is needed kernel-side (as opposed to processor-side)
#[derive(Debug)]
struct SpanState {
    references: AtomicUsize,
    metadata: &'static tracing_core::Metadata<'static>,
}

static QUEUE: StaticThingBuf<Message, 64, recycling::WithCapacity> =
    StaticThingBuf::with_recycle(recycling::WithCapacity::new());

#[derive(Debug)]
struct Message {
    /// Report a serialization error from writing `data`
    error: Option<postcard::Error>,
    /// Serialized event data (may be empty, if there is an error)
    data: heapless::Vec<u8, 1024>,
}

/// Initialize `ktrace` as the `tracing` subscriber.
///
/// The returned worker must be driven periodically for events to be processed.
pub fn init<
    W: Write<Error = Infallible> + Send + 'static,
    TP: platypos_hal::topology::Topology + 'static,
>(
    mut writer: W,
    topology: &'static TP,
) -> Worker<W> {
    writer
        .write_all(&proto::START_OF_OUTPUT)
        .expect("Could not write start-of-output");
    let dispatch = Dispatch::new(KTrace::new(topology));
    tracing_core::dispatcher::set_global_default(dispatch).expect("Tracing initialized twice");
    Worker::new(writer)
}

impl<TP: platypos_hal::topology::Topology + 'static> KTrace<TP> {
    fn new(topology: &'static TP) -> Self {
        KTrace {
            spans: Slab::new(topology),
            // stack: PerProcessor::new(topology),
        }
    }

    /// Current processor ID to report, for contextual spans and events
    fn processor_id(&self) -> proto::ProcessorId {
        0
    }

    /// Handler for fatal internal tracing errors. This is used instead of
    /// `panic!` so that the `panic!` implementation can itself use KTrace.
    fn fatal_error(&self, _msg: &str) -> ! {
        // TODO: indicate error somehow
        loop {}
    }
}

impl<TP: platypos_hal::topology::Topology + 'static> Subscriber for KTrace<TP> {
    fn enabled(&self, _metadata: &tracing_core::Metadata<'_>) -> bool {
        // TODO: filtering directives
        true
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        let Ok(idx) = self.spans.insert(SpanState {
            references: AtomicUsize::new(1),
            metadata: span.metadata()
        }) else {
            self.fatal_error("exceeded max span limit")
        };

        let id = span::Id::from_u64(idx.into());

        let parent = if span.is_root() {
            proto::Parent::Root
        } else if span.is_contextual() {
            proto::Parent::Current(self.processor_id())
        } else {
            // At this point, we know the parent must be set, but avoid panicking
            proto::Parent::Explicit(span.parent().map_or(0, |s| s.into_u64()))
        };

        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.write_message(&proto::Message::SpanCreated(proto::SpanCreated {
                id: idx.into(),
                parent,
                metadata: proto::Metadata::from_tracing(span.metadata()),
                fields: span.into(),
            }));
        }
        // Otherwise, the queue is full - drop this span

        id
    }

    fn record(&self, _span: &span::Id, _values: &span::Record<'_>) {
        todo!()
    }

    fn record_follows_from(&self, _span: &span::Id, _follows: &span::Id) {
        todo!()
    }

    fn event(&self, event: &tracing_core::Event<'_>) {
        let span_id = if event.is_root() {
            proto::Parent::Root
        } else if event.is_contextual() {
            proto::Parent::Current(self.processor_id())
        } else {
            // At this point, we know the parent must be set, but avoid panicking
            proto::Parent::Explicit(event.parent().map_or(0, |s| s.into_u64()))
        };

        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.write_message(&proto::Message::Event(proto::Event {
                span_id,
                metadata: proto::Metadata::from_tracing(event.metadata()),
                fields: event.into(),
            }));
        }
        // Otherwise, the queue is full - drop this event
    }

    fn enter(&self, span: &span::Id) {
        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.write_message(&proto::Message::SpanEntered {
                id: span.into_u64(),
                processor: self.processor_id(),
            });
        }
        // TODO: should probably panic if the queue is full, since tracking will
        // be messed up
    }

    fn exit(&self, span: &span::Id) {
        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.write_message(&proto::Message::SpanExited {
                id: span.into_u64(),
                processor: self.processor_id(),
            });
        }
        // TODO: should probably panic if the queue is full, since tracking will
        // be messed up
    }

    fn clone_span(&self, id: &span::Id) -> span::Id {
        let Some(state) = self.spans.get(id.into_u64().into()) else {
            return id.clone();
        };
        state.references.fetch_add(1, Ordering::Relaxed);
        id.clone()
    }

    fn try_close(&self, id: span::Id) -> bool {
        let idx = id.into_u64().into();
        let Some(state) = self.spans.get(idx) else {
        return false
      };
        let references = state.references.fetch_sub(1, Ordering::Relaxed);

        if references == 1 {
            // This was the last reference
            drop(state);
            self.spans.remove(idx);
            true
        } else {
            false
        }
    }

    // TODO: this would require concurrent access to the span metadata stored in
    // Worker.active_spans fn current_span(&self) -> span::Current {
    // }

    fn max_level_hint(&self) -> Option<tracing_core::LevelFilter> {
        None
    }

    fn event_enabled(&self, _event: &tracing_core::Event<'_>) -> bool {
        true
    }
}

impl Message {
    fn write_message(&mut self, msg: &proto::SenderMessage) {
        // Variant of the postcard HVec flavor that can reuse an existing heapless::Vec
        struct ExistingVec<'a, const B: usize> {
            vec: &'a mut heapless::Vec<u8, B>,
        }

        impl<'a, const B: usize> postcard::ser_flavors::Flavor for ExistingVec<'a, B> {
            type Output = ();

            #[inline(always)]
            fn try_extend(&mut self, data: &[u8]) -> Result<(), postcard::Error> {
                self.vec
                    .extend_from_slice(data)
                    .map_err(|_| postcard::Error::SerializeBufferFull)
            }

            #[inline(always)]
            fn try_push(&mut self, data: u8) -> Result<(), postcard::Error> {
                self.vec
                    .push(data)
                    .map_err(|_| postcard::Error::SerializeBufferFull)
            }

            fn finalize(self) -> Result<Self::Output, postcard::Error> {
                Ok(())
            }
        }

        self.error = postcard::serialize_with_flavor(
            msg,
            ExistingVec {
                vec: &mut self.data,
            },
        )
        .err()
    }
}

// This implements Recycle mainly for clearing behavior, heapless vectors are
// fixed-capacity
impl Recycle<Message> for recycling::WithCapacity {
    fn new_element(&self) -> Message {
        Message {
            error: None,
            data: heapless::Vec::new(),
        }
    }

    fn recycle(&self, element: &mut Message) {
        element.data.clear();
    }
}

impl<W: Write> Worker<W> {
    fn new(writer: W) -> Self {
        Self {
            writer,
            total_events: 0,
        }
    }

    /// Process any queued tracing events
    pub fn work(&mut self) {
        while let Some(event) = QUEUE.pop_ref() {
            self.total_events += 1;
            if let Some(ref err) = event.error {
                self.report_error(err);
            }

            if !event.data.is_empty() {
                // Ignore I/O errors, since there's nowhere to report them anyways
                // TODO: now that data is buffered anyways, use COBS for error recovery
                let _ = self.writer.write_all(&event.data);
            }
        }
    }

    /// Write a locally-produced message from the worker
    fn write_message<const CAP: usize, E: Serialize, A: Serialize>(
        &mut self,
        msg: &proto::Message<'_, E, A>,
    ) {
        match postcard::to_vec::<_, CAP>(msg) {
            Ok(data) => {
                let _ = self.writer.write_all(&data);
            }
            Err(err) => {
                #[cfg(debug_assertions)]
                panic!("Internal write failed: {}", err);
            }
        }
    }

    /// Report a message serialization error
    fn report_error(&mut self, err: &postcard::Error) {
        // let args = format_args!("serialization error: {}", err);
        // let fields = proto::InternalEvent::new(args);
        // let msg: &proto::InternalMessage =
        // &proto::Message::Event(proto::Event {     span_id:
        // proto::Parent::Root,     metadata: proto::Metadata {
        //         name: "<internal tracing error>",
        //         target: "<internal tracing error>",
        //         level: proto::Level::Error,
        //         file: None,
        //         line: None,
        //     },
        //     fields,
        // });
        // self.write_message::<256, _, _>(msg);
    }
}

impl<W: Write> Drop for Worker<W> {
    fn drop(&mut self) {
        // Ensure any queued events are flushed on exit
        self.work();
    }
}
