//! Wrapper around QEMU

use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, ExitStatus, Stdio};

use addr2line::fallible_iterator::FallibleIterator;
use addr2line::{gimli, object, Context};
use camino::Utf8Path;
use color_eyre::eyre::WrapErr;
use color_eyre::Result;
use lazy_static::lazy_static;
use regex::{Captures, Regex};

use crate::platform::Platform;

pub struct Spec<'a> {
    pub binary: &'a Utf8Path,
    pub boot_image: &'a Utf8Path,
    pub platform: Platform,
    pub memory: &'a str,
    pub cpus: usize,
}

/// Creates a new QEMU command for `platform`, including any
/// platform-specific arguments.
fn command_for(platform: Platform) -> Command {
    match platform {
        Platform::X86_64 => {
            let mut command = Command::new("qemu-system-x86_64");
            command.args([
                "-drive",
                "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.fd",
                "-drive",
                "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_VARS.fd",
                "-machine",
                "q35,accel=kvm",
            ]);
            command
        }
    }
}

pub fn run(spec: Spec) -> Result<ExitStatus> {
    let mut cmd = command_for(spec.platform);
    cmd.arg("-drive")
        .arg(format!("format=raw,file={}", spec.boot_image));
    // TODO: fifo for serial console so monitor can use stdio
    cmd.args(["--no-reboot", "-m", spec.memory, "-serial", "stdio"]);
    cmd.arg("-smp").arg(format!("cpus={}", spec.cpus));
    cmd.stdout(Stdio::piped());
    log::debug!("QEMU command: {cmd:?}");

    let mut proc = cmd.spawn().wrap_err("could not start qemu")?;

    let filter = SymbolizeFilter::new(spec.binary)?;
    let stdout = io::stdout().lock();
    filter.drain(BufReader::new(proc.stdout.take().unwrap()), stdout)?;

    proc.wait().wrap_err("could not run qemu")
}

/// Filters the QEMU output to symbolize backtraces
struct SymbolizeFilter {
    context: Context<gimli::EndianRcSlice<gimli::RunTimeEndian>>,
}

lazy_static! {
    static ref SYMBOL_RE: Regex = Regex::new("€€€([0-9a-zA-Z]+)€€€").unwrap();
}

impl SymbolizeFilter {
    fn new(object_file: &Utf8Path) -> Result<SymbolizeFilter> {
        let object_data =
            fs::read(object_file).wrap_err_with(|| format!("could not read {object_file}"))?;

        let object = object::File::parse(&*object_data)
            .wrap_err_with(|| format!("could not parse {object_file}"))?;

        let context = Context::new(&object)?;

        Ok(SymbolizeFilter { context })
    }

    fn symbolize(&self, candidate: &str) -> Result<String> {
        let address = u64::from_str_radix(candidate, 16)?;
        let mut frames = self.context.find_frames(address)?.enumerate();
        let mut buf = String::new();

        while let Some((i, frame)) = frames.next()? {
            if i != 0 {
                write!(buf, "  (inlined by) ")?;
            }

            match frame.function {
                Some(n) => write!(buf, "{}", n.demangle()?)?,
                None => write!(buf, "???")?,
            };

            write!(buf, " at ")?;

            match frame.location {
                Some(loc) => {
                    let file = loc.file.unwrap_or("<unknown>");
                    let line = loc.line.unwrap_or(0);
                    let col = loc.column.unwrap_or(0);
                    write!(buf, "{file}:{line}:{col}")?;
                }
                None => write!(buf, "<unknown>")?,
            }
        }

        Ok(buf)
    }

    fn drain<R: BufRead, W: Write>(&self, src: R, mut dest: W) -> Result<()> {
        for line in src.lines() {
            let line = line?;
            let symbolized = SYMBOL_RE.replace_all(&line, |c: &Captures| {
                self.symbolize(&c[1]).unwrap_or_else(|_| c[1].to_owned())
            });

            writeln!(dest, "{}", symbolized)?;
        }

        Ok(())
    }
}
