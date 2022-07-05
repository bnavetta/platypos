//! Decodes log output from the QEMU guest.
//!
//! This uses defmt with extensions for symbolizing backtraces.

use std::fs;
use std::io::{Read, Write};

use addr2line::gimli;
use camino::Utf8Path;
use color_eyre::eyre::{eyre, Result, WrapErr};
use defmt_decoder::DecodeError;
use lazy_static::lazy_static;

pub struct Decoder {
    context: addr2line::Context<gimli::EndianRcSlice<gimli::RunTimeEndian>>,
    table: defmt_decoder::Table,
    locations: defmt_decoder::Locations,
}

lazy_static! {
    static ref UNKNOWN_PATH: &'static Utf8Path = Utf8Path::new("<unknown>");
}

const START_OF_OUTPUT: [u8; 4] = [255, 0, 255, 0];

impl Decoder {
    pub fn new(binary: &Utf8Path) -> Result<Decoder> {
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
            "{}:{} {} - {}",
            path,
            line,
            module,
            frame.display(true)
        )?;

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
