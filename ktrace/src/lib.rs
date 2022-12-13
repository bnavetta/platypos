//! Kernel Tracing
//!
//! This is an implementation of [`tracing_core`] APIs for use in the PlatypOS
//! kernel.
//!
//! # Goals
//! * Usable from interrupt and memory-allocator contexts. This implies that
//!   tracing entry points cannot allocate and any locks must be
//!   interrupt-aware.
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
use core::sync::atomic::{AtomicU64, Ordering};

use hashbrown::hash_map::Entry;
use platypos_hal::Write;
use platypos_ktrace_proto as proto;

use hashbrown::HashMap;
use serde::Serialize;
use thingbuf::recycling::{self, Recycle};
use thingbuf::StaticThingBuf;
use tracing_core::{span, Dispatch, Subscriber};

mod slab;
mod stack;
mod sync;

pub struct KTrace {
    next_id: AtomicU64,
}

pub struct Worker<W: Write> {
    writer: W,
    active_spans: HashMap<span::Id, SpanState>,
    total_events: usize,
}

/// Per-span state that is needed kernel-side (as opposed to processor-side)
#[derive(Debug)]
struct SpanState {
    references: usize,
    // TODO: this isn't used since all state is tracked processor-side in the current design
    //   we could do more locally if spans were accessible in KTrace
    #[allow(dead_code)]
    metadata: &'static tracing_core::Metadata<'static>,
}

static QUEUE: StaticThingBuf<Message, 64, recycling::WithCapacity> =
    StaticThingBuf::with_recycle(recycling::WithCapacity::new());

#[derive(Debug)]
struct Message {
    command: Option<Command>,
    /// Report a serialization error from writing `data`
    error: Option<postcard::Error>,
    /// Serialized event data (may be empty, depending on the [`command`])
    data: heapless::Vec<u8, 1024>,
}

/// Instructions passed to the worker task, along with message data to send
#[derive(Debug)]
enum Command {
    New {
        id: span::Id,
        metadata: &'static tracing_core::Metadata<'static>,
    },
    Reference(span::Id),
    Dereference(span::Id),
}

/// Initialize `ktrace` as the `tracing` subscriber.
///
/// The returned worker must be driven periodically for events to be processed.
pub fn init<W: Write<Error = Infallible> + Send + 'static>(mut writer: W) -> Worker<W> {
    writer
        .write_all(&proto::START_OF_OUTPUT)
        .expect("Could not write start-of-output");
    let dispatch = Dispatch::new(KTrace::new());
    tracing_core::dispatcher::set_global_default(dispatch).expect("Tracing initialized twice");
    Worker::new(writer)
}

impl KTrace {
    fn new() -> Self {
        KTrace {
            next_id: AtomicU64::new(1),
        }
    }

    /// Current processor ID to report, for contextual spans and events
    fn processor_id(&self) -> proto::ProcessorId {
        0
    }
}

impl Subscriber for KTrace {
    fn enabled(&self, _metadata: &tracing_core::Metadata<'_>) -> bool {
        // TODO: filtering directives
        true
    }

    fn new_span(&self, span: &span::Attributes<'_>) -> span::Id {
        let mut id = self.next_id.fetch_add(1, Ordering::Relaxed);
        if id == 0 {
            // Wrapped around!
            id = self.next_id.fetch_add(1, Ordering::Relaxed);
            debug_assert!(id != 0);
        }

        let parent = if span.is_root() {
            proto::Parent::Root
        } else if span.is_contextual() {
            proto::Parent::Current(self.processor_id())
        } else {
            // At this point, we know the parent must be set, but avoid panicking
            proto::Parent::Explicit(span.parent().map_or(0, |s| s.into_u64()))
        };

        // Safety: we ensure that id is nonzero above

        let span_id = span::Id::from_non_zero_u64(unsafe { NonZeroU64::new_unchecked(id) });

        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.command = Some(Command::New {
                id: span_id.clone(),
                metadata: span.metadata(),
            });
            slot.write_message(&proto::Message::SpanCreated(proto::SpanCreated {
                id,
                parent,
                metadata: proto::Metadata::from_tracing(span.metadata()),
                fields: span.into(),
            }));
        }
        // Otherwise, the queue is full - drop this span

        span_id
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
        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.command = Some(Command::Reference(id.clone()))
        }
        // TODO: should probably panic if the queue is full, since tracking will be
        // messed up
        id.clone()
    }

    fn try_close(&self, id: span::Id) -> bool {
        if let Ok(mut slot) = QUEUE.push_ref() {
            slot.command = Some(Command::Dereference(id))
        }

        // At this point, we don't _know_ if all referenced have been closed, but it
        // appears that nothing relies on the return value Consider using a
        // sharded slab or other synchronizable data structure for span state, while
        // keeping the queue for I/O
        false
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
            command: None,
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
            active_spans: HashMap::new(),
            total_events: 0,
        }
    }

    /// Process any queued tracing events
    pub fn work(&mut self) {
        while let Some(event) = QUEUE.pop_ref() {
            self.total_events += 1;
            match &event.command {
                Some(Command::New { id, metadata }) => {
                    // If the span ID counter has wrapped around, this will overwrite previous
                    // spans. The assumption is that they're old and unlikely to be active (for
                    // example, if there's a bug where some span never gets closed.)
                    self.active_spans.insert(
                        id.clone(),
                        SpanState {
                            references: 1,
                            metadata,
                        },
                    );
                }
                Some(Command::Reference(id)) => {
                    if let Some(state) = self.active_spans.get_mut(id) {
                        state.references += 1;
                    }
                    // Silently ignore referencing an unknown span
                }
                Some(Command::Dereference(id)) => {
                    if let Entry::Occupied(mut entry) = self.active_spans.entry(id.clone()) {
                        entry.get_mut().references -= 1;
                        if entry.get().references == 0 {
                            entry.remove();
                            // self.write_message::<16, _,
                            // _>(&proto::Message::SpanClosed {
                            //     id: id.into_u64(),
                            // });
                        }
                    }
                    // Silently ignore dereferencing an unknown span
                }
                None => (),
            }

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
