use std::io::{Read, Write};

use color_eyre::eyre::bail;
use color_eyre::Result;
use platypos_ktrace_proto::{ReceiverMessage, MAX_MESSAGE_SIZE, START_OF_OUTPUT};

pub mod fmt;

// Unfortunately, we can't use postcard's CobsAccumulator helper because of the
// lifetime requirements on Message - it's only valid while we still have the
// specific buffer it was read from.
// Decoder more or less mirrors how CobsAccumulator works

/// Decoder for COBS-encoded messages
pub struct Decoder {
    buf: [u8; MAX_MESSAGE_SIZE],
    index: usize,
    read_header: bool,
}

impl Decoder {
    pub fn new() -> Self {
        Self {
            buf: [0u8; MAX_MESSAGE_SIZE],
            index: 0,
            read_header: false,
        }
    }

    pub fn read_to_header<R: Read, W: Write>(
        &mut self,
        input: &mut R,
        output: &mut W,
    ) -> Result<()> {
        let mut input_buf = [0u8; 64];
        let finder = memchr::memmem::Finder::new(&START_OF_OUTPUT);
        let mut data = Vec::new();

        loop {
            let count = input.read(&mut input_buf)?;
            if count == 0 {
                bail!("Could not find start-of-output marker");
            }

            data.extend_from_slice(&input_buf[..count]);

            if let Some(pos) = finder.find(&data[..]) {
                output.write_all(&data[..pos])?;
                self.extend(&data[pos + START_OF_OUTPUT.len()..])?;
                self.read_header = true;
                break;
            }
        }

        Ok(())
    }

    pub fn decode<R, F>(&mut self, mut input: R, mut f: F) -> Result<()>
    where
        R: Read,
        F: FnMut(ReceiverMessage) -> Result<()>,
    {
        if !self.read_header {
            bail!("Have not read start-of-output header");
        }

        let mut input_buf = [0u8; 64];
        loop {
            let count = input.read(&mut input_buf)?;
            if count == 0 {
                break;
            }

            // The slice of data we've just received
            let mut input = &input_buf[..count];

            // Process every complete message we might now have
            'cobs: while !input.is_empty() {
                // look for the end-of-message zero byte
                let zero_pos = input.iter().position(|&i| i == 0);
                if let Some(n) = zero_pos {
                    // Find the bytes that are part of this message and the next one
                    let (take, release) = input.split_at(n + 1);

                    // Add the end of the message to the state buffer
                    self.extend(take)?;

                    match postcard::from_bytes_cobs(&mut self.buf[..self.index]) {
                        Ok(msg) => f(msg)?,
                        Err(err) => log::error!("Malformed message: {err}"),
                    };

                    self.index = 0;
                    input = release;
                } else {
                    // Didn't read an end-of-message so add it to the buffer
                    self.extend(input)?;
                    break 'cobs;
                }
            }
        }

        Ok(())
    }

    fn extend(&mut self, data: &[u8]) -> Result<()> {
        let needed = self.index + data.len();
        if needed > MAX_MESSAGE_SIZE {
            bail!("{needed}-byte state exceeded maximum of {MAX_MESSAGE_SIZE}");
        }

        let new_end = self.index + data.len();
        self.buf[self.index..new_end].copy_from_slice(data);
        self.index = new_end;
        Ok(())
    }
}

impl Default for Decoder {
    fn default() -> Self {
        Self::new()
    }
}

// TODO: track span state
