use std::{fmt, fs};

use addr2line::fallible_iterator::FallibleIterator;
use addr2line::gimli;
use platypos_ktrace_decoder::fmt::Symbolizer;

use crate::prelude::*;

pub(crate) struct GimliSymbolizer {
    context: addr2line::Context<gimli::EndianRcSlice<gimli::RunTimeEndian>>,
}

impl GimliSymbolizer {
    pub(crate) fn new(binary: &Utf8Path) -> Result<Self> {
        let data = fs::read(binary).wrap_err_with(|| format!("could not read {binary}"))?;

        let object = addr2line::object::File::parse(&*data)
            .wrap_err_with(|| format!("could not parse {binary}"))?;

        let context = addr2line::Context::new(&object)?;

        Ok(Self { context })
    }
}

impl<'a> Symbolizer for &'a GimliSymbolizer {
    fn symbolize(&self, address: u64, f: &mut fmt::Formatter) -> fmt::Result {
        // Note: when the kernel was higher-half, there was a weird issue where
        // all the addresses had to be moved up by 0xffffffff00000000 for
        // symbolization to work. It seems like DWARF and llvm-unwind
        // (the backend for the mini-backtrace crate) disagree on how to
        // handle a higher-half kernel. This could be something to do
        // with how the kernel code model works, or a mistake I made in
        // the linker script. Either way, to correctly find symbols we have to
        // shift the address up.

        let mut wrote_frame = false;

        if let Ok(frames) = self.context.find_frames(address) {
            let mut frames = frames.enumerate();
            loop {
                match frames.next() {
                    Ok(Some((i, frame))) => {
                        wrote_frame = true;
                        if i != 0 {
                            write!(f, " (inlined by) ")?;
                        }

                        match frame.function {
                            Some(name) => match name.demangle() {
                                Ok(n) => f.write_str(&n)?,
                                Err(_) => write!(f, "???")?,
                            },
                            None => write!(f, "???")?,
                        }

                        write!(f, " @ ")?;

                        match frame.location {
                            Some(loc) => {
                                let file = loc.file.unwrap_or("<unknown>");
                                let line = loc.line.unwrap_or(0);
                                let col = loc.column.unwrap_or(0);
                                write!(f, "{file}:{line}:{col}")?;
                            }
                            None => write!(f, "<unknown>")?,
                        }
                    }
                    Ok(None) => break,
                    Err(e) => {
                        if wrote_frame {
                            write!(f, " (inlined by) ")?;
                        }
                        wrote_frame = true;
                        write!(f, "<malformed: {e}>")?;
                    }
                }
            }
        }

        if !wrote_frame {
            write!(f, "<unknown symbol @ {address:#012x}>")?;
        }

        Ok(())
    }
}
