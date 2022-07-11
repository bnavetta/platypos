use std::collections::VecDeque;
use std::io::{Read, Write};

use color_eyre::eyre::bail;
use color_eyre::Result;
use platypos_ktrace_proto::{ReceiverMessage, START_OF_OUTPUT};

pub mod fmt;

/// Decoder for ktrace messages
pub struct Decoder {
    buf: VecDeque<u8>,
    read_header: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            buf: VecDeque::new(),
            read_header: false,
        }
    }

    /// Reads from `input` until the marker for the start of ktrace output is
    /// found, writing non-ktrace data to `output`
    fn read_initial<R: Read, W: Write>(&mut self, input: &mut R, output: &mut W) -> Result<()> {
        let mut input_buf = [0u8; 64];
        let finder = memchr::memmem::Finder::new(&START_OF_OUTPUT);

        loop {
            let count = input.read(&mut input_buf)?;
            if count == 0 {
                bail!("could not find ktrace marker");
            }

            self.buf.extend(&input_buf[..count]);
            let slice = self.buf.make_contiguous();
            if let Some(pos) = finder.find(slice.as_ref()) {
                output.write_all(&slice[..pos])?;
                self.read_header = true;
                self.buf.drain(..pos + START_OF_OUTPUT.len());
                break;
            } else if slice.len() > START_OF_OUTPUT.len() {
                // Write out the data we know won't be part of the marker
                let to_write = slice.len() - START_OF_OUTPUT.len();
                output.write_all(&slice[..to_write])?;
                self.buf.drain(..to_write);
            }
        }

        Ok(())
    }

    pub fn decode<R, W, F>(&mut self, mut input: R, mut drain: W, mut f: F) -> Result<()>
    where
        R: Read,
        W: Write,
        F: FnMut(ReceiverMessage) -> Result<()>,
    {
        self.read_initial(&mut input, &mut drain)?;
        drop(drain); // In case it's locked stdout

        let mut input_buf = [0u8; 64];
        loop {
            let count = input.read(&mut input_buf)?;
            if count == 0 {
                break;
            }

            self.buf.extend(&input_buf[..count]);
            self.buf.make_contiguous();
            'decode: loop {
                let slice = match self.buf.as_slices() {
                    (slice, &[]) => slice,
                    _ => panic!("data not contiguous"),
                };
                match postcard::take_from_bytes(slice) {
                    Err(postcard::Error::DeserializeUnexpectedEnd) => break 'decode,
                    Err(other) => return Err(other.into()),
                    Ok((msg, unused)) => {
                        f(msg)?;
                        // Drain off the data that was used
                        let used = slice.len() - unused.len();
                        self.buf.drain(..used);
                    }
                }
            }
        }

        Ok(())
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}
