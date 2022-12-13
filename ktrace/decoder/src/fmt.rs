//! Stateful pretty-printer for ktrace

use std::collections::HashMap;
use std::fmt;
use std::fmt::Display;

use owo_colors::{OwoColorize, Stream};
use platypos_ktrace_proto as proto;

pub struct Formatter<S: Symbolizer> {
    spans: HashMap<proto::SpanId, SpanState>,
    span_stacks: HashMap<proto::ProcessorId, Vec<proto::SpanId>>,
    symbolizer: S,
}

/// Interface for resolving `KernelAddress` values into symbols.
pub trait Symbolizer {
    fn symbolize(&self, address: u64, f: &mut fmt::Formatter) -> fmt::Result;
}

impl<S: Symbolizer> Formatter<S> {
    pub fn new(symbolizer: S) -> Self {
        Formatter {
            spans: HashMap::new(),
            span_stacks: HashMap::new(),
            symbolizer,
        }
    }

    fn stack(&mut self, processor: proto::ProcessorId) -> &mut Vec<proto::SpanId> {
        self.span_stacks.entry(processor).or_default()
    }

    fn resolve_parent(&mut self, parent: &proto::Parent) -> Option<proto::SpanId> {
        match parent {
            proto::Parent::Root => None,
            proto::Parent::Current(processor) => self.stack(*processor).last().cloned(),
            proto::Parent::Explicit(id) => Some(*id),
        }
    }

    pub fn receive(&mut self, message: &proto::ReceiverMessage) {
        match message {
            proto::Message::SpanCreated(span) => {
                let parent = self
                    .resolve_parent(&span.parent)
                    .and_then(|s| self.spans.get(&s));
                let depth = parent.map_or(0, |s| s.depth + 1);

                let state = SpanState {
                    id: span.id,
                    depth,
                    name: span.metadata.name.to_string(),
                    target: span.metadata.target.to_string(),
                    level: span.metadata.level,
                };
                print!(
                    "{}╔ {} {}",
                    Indent::spaces(depth),
                    LevelColor(span.metadata.level, span.metadata.level),
                    state.name()
                );
                if let Some(parent) = parent {
                    print!(" ⇜ {}", parent.name());
                }
                println!();
                if !span.fields.is_empty() {
                    println!(
                        "{}  {}",
                        Indent::spaces(depth),
                        DisplayFields {
                            fields: &span.fields,
                            depth: depth + 2,
                            symbolizer: &self.symbolizer,
                        }
                    );
                }
                self.spans.insert(span.id, state);
            }
            proto::Message::Event(event) => {
                let depth = self
                    .resolve_parent(&event.span_id)
                    .and_then(|s| self.spans.get(&s))
                    .map_or(0, |s| s.depth)
                    + 1;
                println!(
                    "{}└ {} {}",
                    Indent::spaces(depth),
                    LevelColor(event.metadata.level, event.metadata.level),
                    DisplayFields {
                        fields: &event.fields,
                        depth: depth + 1,
                        symbolizer: &self.symbolizer,
                    }
                );
            }
            proto::Message::SpanClosed { id } => {
                if let Some(span) = self.spans.remove(id) {
                    println!(
                        "{}╚ {} {}",
                        Indent::spaces(span.depth),
                        LevelColor(span.level, "END"),
                        span.name()
                    )
                }
            }
            proto::Message::SpanEntered { id, processor } => {
                self.stack(*processor).push(*id);
            }
            proto::Message::SpanExited { id, processor } => {
                let prev = self.stack(*processor).pop();
                assert!(prev == Some(*id), "Exited span was not current!");
            }
        }
    }
}

// TODO: this clutters the output too much
#[allow(dead_code)]
fn write_location(depth: usize, metadata: &proto::Metadata) {
    if let Some(file) = metadata.file {
        print!("{}  @ {}", Indent::spaces(depth), file);
        if let Some(line) = metadata.line {
            println!(":{}", line);
        } else {
            println!();
        }
    }
}

struct SpanState {
    depth: usize,
    target: String,
    name: String,
    level: proto::Level,
    id: u64,
}

impl SpanState {
    fn name(&self) -> SpanName {
        SpanName(self)
    }
}

struct SpanName<'a>(&'a SpanState);

impl<'a> fmt::Display for SpanName<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}#{}", self.0.target, self.0.name, self.0.id)
    }
}

/// Colorize a value based on a trace level
struct LevelColor<D: fmt::Display>(proto::Level, D);

impl<D: fmt::Display> fmt::Display for LevelColor<D> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            proto::Level::Error => {
                write!(
                    f,
                    "{}",
                    self.1.if_supports_color(Stream::Stdout, |l| l.red())
                )
            }
            proto::Level::Warn => write!(
                f,
                "{}",
                self.1.if_supports_color(Stream::Stdout, |l| l.yellow())
            ),
            proto::Level::Info => write!(
                f,
                "{}",
                self.1.if_supports_color(Stream::Stdout, |l| l.green())
            ),
            proto::Level::Debug => write!(
                f,
                "{}",
                self.1.if_supports_color(Stream::Stdout, |l| l.blue())
            ),
            proto::Level::Trace => write!(
                f,
                "{}",
                self.1.if_supports_color(Stream::Stdout, |l| l.dimmed())
            ),
        }
    }
}

struct DisplayFields<'a, S: Symbolizer> {
    fields: &'a proto::DeserializedFields<'a>,
    // TODO: replace these with a context type
    depth: usize,
    symbolizer: &'a S,
}

impl<'a, S: Symbolizer> fmt::Display for DisplayFields<'a, S> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut is_first = true;

        for (name, value) in self.fields.iter() {
            if !is_first {
                write!(f, " ")?;
            }
            is_first = false;

            if *name != "message" {
                write!(
                    f,
                    "{}: ",
                    name.if_supports_color(Stream::Stdout, |n| n.bold())
                )?;
            }

            write_value(value, f, self.depth, self.symbolizer)?;
        }

        Ok(())
    }
}

fn write_value<S: Symbolizer>(
    value: &proto::Value<'_>,
    f: &mut fmt::Formatter,
    depth: usize,
    symbolizer: &S,
) -> fmt::Result {
    match value {
        proto::Value::KernelAddress(address) => symbolizer.symbolize(*address, f),
        proto::Value::String(s) => {
            let mut is_first = true;
            let mut lines = s.lines().peekable();

            while let Some(line) = lines.next() {
                if !is_first {
                    write!(f, "{}", Indent::spaces(depth))?;
                }
                f.write_str(line)?;
                is_first = false;

                if lines.peek().is_some() {
                    f.write_str("\n")?;
                }
            }

            Ok(())
        }
        proto::Value::U64(x) => write!(f, "{x}"),
        // Color-code addresses so they can be disambiguated
        proto::Value::PhysicalAddress(addr) => format_args!("{addr:#012x}")
            .if_supports_color(Stream::Stdout, |a| a.magenta())
            .fmt(f),
        proto::Value::VirtualAddress(addr) => format_args!("{addr:#012x}")
            .if_supports_color(Stream::Stdout, |a| a.cyan())
            .fmt(f),
    }
}

struct Indent<'a>(&'a str, usize);

impl<'a> Indent<'a> {
    pub fn spaces(depth: usize) -> Indent<'static> {
        Indent("  ", depth)
    }
}

impl<'a> fmt::Display for Indent<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for _ in 0..self.1 {
            f.write_str(self.0)?;
        }
        Ok(())
    }
}
