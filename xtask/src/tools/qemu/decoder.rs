//! Decodes log output from the QEMU guest.
//!
//! This uses defmt with extensions for symbolizing backtraces.

use std::io::{Read, Write};
use std::{fs, mem};

use addr2line::fallible_iterator::FallibleIterator;
use addr2line::gimli;
use camino::Utf8Path;
use color_eyre::eyre::{eyre, Result, WrapErr};
use defmt_decoder::{Arg, DecodeError};
use defmt_parser::{DisplayHint, Level, ParserMode, Type};
use lazy_static::lazy_static;
use owo_colors::OwoColorize;

use crate::prelude::Platform;

pub struct Decoder {
    platform: Platform,
    context: addr2line::Context<gimli::EndianRcSlice<gimli::RunTimeEndian>>,
    table: defmt_decoder::Table,
    locations: defmt_decoder::Locations,
}

lazy_static! {
    static ref UNKNOWN_PATH: &'static Utf8Path = Utf8Path::new("<unknown>");
}

const START_OF_OUTPUT: [u8; 4] = [255, 0, 255, 0];

impl Decoder {
    pub fn new(platform: Platform, binary: &Utf8Path) -> Result<Decoder> {
        let data = fs::read(binary).wrap_err_with(|| format!("could not read {binary}"))?;

        let object = addr2line::object::File::parse(&*data)
            .wrap_err_with(|| format!("could not parse {binary}"))?;

        let context = addr2line::Context::new(&object)?;

        let table = defmt_decoder::Table::parse(&*data)
            .map_err(|e| eyre!("could not parse defmt data in {binary}: {e}"))?
            .ok_or_else(|| eyre!("could not find defmt data in {binary}"))?;

        let locations = table
            .get_locations(&data)
            .map_err(|e| eyre!("could not read defmt location data in {binary}: {e}"))?;

        Ok(Decoder {
            platform,
            context,
            table,
            locations,
        })
    }

    pub fn decode<W: Write, R: Read>(&self, mut src: R, mut dest: W) -> Result<()> {
        let mut buf = [0; 1024];
        let mut stream_decoder = self.table.new_stream_decoder();

        stream_decoder.received(&self.read_until_defmt(&mut src, &mut dest)?);

        loop {
            let count = src.read(&mut buf)?;
            if count == 0 {
                break Ok(());
            }

            stream_decoder.received(&buf[..count]);

            loop {
                match stream_decoder.decode() {
                    Ok(frame) => self.log_frame(&frame, &mut dest)?,
                    Err(DecodeError::UnexpectedEof) => break,
                    Err(DecodeError::Malformed) => {
                        if self.table.encoding().can_recover() {
                            writeln!(dest, "< Skipping malformed frame >")?;
                        } else {
                            return Err(DecodeError::Malformed.into());
                        }
                    }
                }
            }
        }
    }

    fn log_frame<W: Write>(&self, frame: &defmt_decoder::Frame, dest: &mut W) -> Result<()> {
        if let Some(ts) = frame.display_timestamp() {
            write!(dest, "{} ", ts)?;
        }

        if let Some(level) = frame.level() {
            match level {
                Level::Trace => write!(dest, "{} ", "TRACE".dimmed())?,
                Level::Debug => write!(dest, "DEBUG ")?,
                Level::Info => write!(dest, "{} ", "INFO".green())?,
                Level::Warn => write!(dest, "{} ", "WARN".yellow())?,
                Level::Error => write!(dest, "{} ", "ERROR".red())?,
            }
        }

        self.format_to(frame.format(), frame.args(), None, dest)?;
        writeln!(dest)?;

        let location = self.locations.get(&frame.index());
        let (path, line, module) = match location {
            Some(loc) => (
                Utf8Path::from_path(loc.file.as_path()).unwrap_or(&UNKNOWN_PATH),
                loc.line,
                &*loc.module,
            ),
            None => (&**UNKNOWN_PATH, 0, "<unknown>"),
        };

        writeln!(
            dest,
            "{}",
            format!("└─ {} @ {}:{}", module, path, line).dimmed()
        )?;

        Ok(())
    }

    /// Write a (modified) defmt string. This is a massive abuse of defmt
    /// internals, but does allow custom formatting.
    fn format_to(
        &self,
        format: &str,
        args: &[Arg],
        parent_hint: Option<&DisplayHint>,
        dest: &mut dyn Write,
    ) -> Result<()> {
        // Largely adapted from https://github.com/knurling-rs/defmt/blob/7886fa030b133234db39aba5ce976e97155eb8f0/decoder/src/frame.rs#L106
        let params = defmt_parser::parse(format, ParserMode::ForwardsCompatible)
            .map_err(|e| eyre!("defmt parse error: {e}"))?;

        for param in params {
            match param {
                defmt_parser::Fragment::Literal(lit) => write!(dest, "{}", lit)?,
                defmt_parser::Fragment::Parameter(param) => {
                    let hint = param.hint.as_ref().or(parent_hint);

                    match &args[param.index] {
                        Arg::Bool(x) => write!(dest, "{}", x)?,
                        Arg::F32(x) => write!(dest, "{}", x)?,
                        Arg::F64(x) => write!(dest, "{}", x)?,
                        Arg::Uxx(x) => match param.ty {
                            Type::BitField(range) => {
                                let left_zeroes = mem::size_of::<u128>() * 8 - range.end as usize;
                                let right_zeroes = left_zeroes + range.start as usize;
                                // isolate the desired bitfields
                                let bitfields = (*x << left_zeroes) >> right_zeroes;

                                if let Some(DisplayHint::Ascii) = hint {
                                    let bstr = bitfields
                                        .to_be_bytes()
                                        .iter()
                                        .skip(right_zeroes / 8)
                                        .copied()
                                        .collect::<Vec<u8>>();
                                    self.format_bytes(&bstr, hint, dest)?
                                } else {
                                    self.format_u128(bitfields as u128, hint, dest)?;
                                }
                            }
                            _ => match hint {
                                Some(DisplayHint::ISO8601(_)) => todo!(),
                                Some(DisplayHint::Debug) => {
                                    self.format_u128(*x, parent_hint, dest)?
                                }
                                _ => self.format_u128(*x, hint, dest)?,
                            },
                        },
                        Arg::Ixx(x) => self.format_i128(*x, hint, dest)?,
                        Arg::Str(x) | Arg::Preformatted(x) => self.format_str(x, hint, dest)?,
                        Arg::IStr(x) => self.format_str(x, hint, dest)?,
                        Arg::Format { format, args } => match parent_hint {
                            Some(DisplayHint::Ascii) => {
                                self.format_to(format, args, parent_hint, dest)?
                            }
                            _ => self.format_to(format, args, hint, dest)?,
                        },
                        Arg::FormatSequence { args } => {
                            for arg in args {
                                self.format_to("{=?}", &[arg.clone()], hint, dest)?;
                            }
                        }
                        Arg::FormatSlice { elements } => {
                            // Skipping the special case for ASCII bytes
                            write!(dest, "[")?;
                            let mut is_first = true;
                            for element in elements {
                                if !is_first {
                                    write!(dest, ", ")?;
                                }
                                is_first = false;
                                self.format_to(element.format, &element.args, hint, dest)?;
                            }
                            write!(dest, "]")?;
                        }
                        Arg::Slice(x) => self.format_bytes(x, hint, dest)?,
                        Arg::Char(c) => write!(dest, "{}", c)?,
                    }
                }
            }
        }

        Ok(())
    }

    fn format_u128(&self, x: u128, hint: Option<&DisplayHint>, dest: &mut dyn Write) -> Result<()> {
        match hint {
            Some(DisplayHint::NoHint { zero_pad }) => write!(dest, "{:01$}", x, zero_pad)?,
            Some(DisplayHint::Unknown(h)) if h == "address" => {
                self.format_symbol(x as u64, dest)?
            }
            Some(DisplayHint::Binary {
                alternate,
                zero_pad,
            }) => match alternate {
                true => write!(dest, "{:#01$b}", x, zero_pad)?,
                false => write!(dest, "{:01$b}", x, zero_pad)?,
            },
            Some(DisplayHint::Hexadecimal {
                alternate,
                uppercase,
                zero_pad,
            }) => match (alternate, uppercase) {
                (false, false) => write!(dest, "{:01$x}", x, zero_pad),
                (false, true) => write!(dest, "{:01$X}", x, zero_pad),
                (true, false) => write!(dest, "{:#01$x}", x, zero_pad),
                (true, true) => write!(dest, "{:#01$X}", x, zero_pad),
            }?,
            Some(DisplayHint::Microseconds) => {
                let seconds = x / 1_000_000;
                let micros = x % 1_000_000;
                write!(dest, "{}.{:06}", seconds, micros)?;
            }
            Some(DisplayHint::Bitflags { .. }) => todo!(),
            _ => write!(dest, "{}", x)?,
        }

        Ok(())
    }

    fn format_i128(&self, x: i128, hint: Option<&DisplayHint>, dest: &mut dyn Write) -> Result<()> {
        match hint {
            Some(DisplayHint::NoHint { zero_pad }) => write!(dest, "{:01$}", x, zero_pad)?,
            Some(DisplayHint::Binary {
                alternate,
                zero_pad,
            }) => match alternate {
                true => write!(dest, "{:#01$b}", x, zero_pad)?,
                false => write!(dest, "{:01$b}", x, zero_pad)?,
            },
            Some(DisplayHint::Hexadecimal {
                alternate,
                uppercase,
                zero_pad,
            }) => match (alternate, uppercase) {
                (false, false) => write!(dest, "{:01$x}", x, zero_pad),
                (false, true) => write!(dest, "{:01$X}", x, zero_pad),
                (true, false) => write!(dest, "{:#01$x}", x, zero_pad),
                (true, true) => write!(dest, "{:#01$X}", x, zero_pad),
            }?,
            _ => write!(dest, "{}", x)?,
        }

        Ok(())
    }

    fn format_bytes(
        &self,
        bytes: &[u8],
        hint: Option<&DisplayHint>,
        dest: &mut dyn Write,
    ) -> Result<()> {
        match hint {
            Some(DisplayHint::Ascii) => {
                write!(dest, "b\"")?;
                for byte in bytes {
                    match byte {
                        b'\t' => write!(dest, "\\t")?,
                        b'\n' => write!(dest, "\\n")?,
                        b'\r' => write!(dest, "\\r")?,
                        b'\"' => write!(dest, "\\\"")?,
                        b'\\' => write!(dest, "\\\\")?,
                        _ => {
                            if byte.is_ascii_graphic() {
                                dest.write_all(&[*byte])?;
                            } else {
                                write!(dest, "\\x{:02x}", byte)?;
                            }
                        }
                    }
                }
            }
            Some(DisplayHint::Hexadecimal { .. } | DisplayHint::Binary { .. }) => {
                write!(dest, "[")?;
                let mut is_first = true;
                for byte in bytes {
                    if !is_first {
                        write!(dest, ", ")?;
                    }
                    is_first = false;
                    self.format_u128(*byte as u128, hint, dest)?;
                }
                write!(dest, "]")?;
            }
            _ => write!(dest, "{:?}", bytes)?,
        }

        Ok(())
    }

    fn format_str(&self, s: &str, hint: Option<&DisplayHint>, dest: &mut dyn Write) -> Result<()> {
        if hint == Some(&DisplayHint::Debug) {
            write!(dest, "{:?}", s)?;
        } else {
            write!(dest, "{}", s)?;
        }
        Ok(())
    }

    fn format_symbol(&self, address: u64, dest: &mut dyn Write) -> Result<()> {
        let adjusted_address = {
            let Platform::X86_64 = self.platform; // Will fail if we have to handle other platforms

            // It seems like DWARF and llvm-unwind (the backend for the mini-backtrace
            // crate) disagree on how to handle a higher-half kernel. This could be
            // something to do with how the kernel code model works, or a mistake I made in
            // the linker script. Either way, to correctly find symbols we have to shift the
            // address up.
            address + 0xffffffff00000000
        };

        let mut frames = self.context.find_frames(adjusted_address)?.enumerate();

        while let Some((i, frame)) = frames.next()? {
            if i != 0 {
                write!(dest, " (inlined by) ")?;
            }

            match frame.function {
                Some(name) => write!(dest, "{}", name.demangle()?)?,
                None => write!(dest, "???")?,
            }

            write!(dest, " at ")?;
            // TODO: something about defmt breaks addr2line (and llvm-addr2line)

            match frame.location {
                Some(loc) => {
                    let file = loc.file.unwrap_or("<unknown>");
                    let line = loc.line.unwrap_or(0);
                    let col = loc.column.unwrap_or(0);
                    write!(dest, "{file}:{line}:{col}")?;
                }
                None => write!(dest, "<unknown>")?,
            }
        }

        Ok(())
    }

    /// Scans over the input stream until it finds the marker for defmt output,
    /// returning any extra bytes that should be fed into the defmt decoder.
    fn read_until_defmt<W: Write, R: Read>(&self, src: &mut R, dest: &mut W) -> Result<Vec<u8>> {
        let mut data = Vec::new();
        let mut buf = [0; 1024];
        let finder = memchr::memmem::Finder::new(&START_OF_OUTPUT);

        // This is not terribly efficient on several levels, but since we don't expect
        // all that much output from the bootloader it's fine.
        loop {
            let read = src.read(&mut buf)?;
            data.extend_from_slice(&buf[0..read]);

            if let Some(pos) = finder.find(&data) {
                // Write the bootloader output
                dest.write_all(&data[0..pos])?;
                dest.flush()?;

                data.drain(0..pos + START_OF_OUTPUT.len());
                break Ok(data);
            }
        }
    }
}
