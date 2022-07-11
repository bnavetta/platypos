#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use core::fmt;

use serde::{Deserialize, Serialize};

mod fields;

pub use fields::{DeserializedFields, FieldType, Value};

/// Marker written by the kernel to indicate that it's started writing to the
/// serial port (and not the bootloader).
pub const START_OF_OUTPUT: [u8; 4] = [255, 0, 255, 0];

pub type SenderMessage<'a> =
    Message<'a, fields::SerializeEvent<'a>, fields::SerializeAttributes<'a>>;

pub type ReceiverMessage<'a> =
    Message<'a, fields::DeserializedFields<'a>, fields::DeserializedFields<'a>>;

/// Root type for KTrace messages
#[derive(Deserialize, Serialize, Debug)]
pub enum Message<'a, E, A> {
    SpanCreated(#[serde(borrow)] SpanCreated<'a, A>),
    Event(#[serde(borrow)] Event<'a, E>),

    /// A span has been closed, so it can no longer be entered
    SpanClosed {
        id: u64,
    },
}

/// A new span was created
#[derive(Deserialize, Serialize, Debug)]
pub struct SpanCreated<'a, A> {
    pub id: u64,
    pub parent: Option<u64>,

    #[serde(borrow)]
    pub metadata: Metadata<'a>,

    pub fields: A,
}

/// A tracing event occurred
#[derive(Deserialize, Serialize, Debug)]
pub struct Event<'a, E> {
    pub span_id: Option<u64>,

    #[serde(borrow)]
    pub metadata: Metadata<'a>,

    pub fields: E,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Metadata<'a> {
    pub name: &'a str,
    pub target: &'a str,
    pub level: Level,

    pub file: Option<&'a str>,
    pub line: Option<u32>,
}

impl<'a> Metadata<'a> {
    pub fn from_tracing(m: &tracing::Metadata<'a>) -> Metadata<'a> {
        Metadata {
            name: m.name(),
            target: m.target(),
            level: m.level().into(),
            file: m.file(),
            line: m.line(),
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Clone, Copy)]
pub enum Level {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl From<&tracing::Level> for Level {
    fn from(t: &tracing::Level) -> Self {
        match *t {
            tracing::Level::ERROR => Level::Error,
            tracing::Level::WARN => Level::Warn,
            tracing::Level::INFO => Level::Info,
            tracing::Level::DEBUG => Level::Debug,
            tracing::Level::TRACE => Level::Trace,
        }
    }
}

impl fmt::Display for Level {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(match self {
            Level::Error => "ERROR",
            Level::Warn => "WARN",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        })
    }
}
