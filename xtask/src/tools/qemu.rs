//! Wrapper around QEMU

use std::fmt::Write as _;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::process::{Command, ExitStatus, Stdio};
use std::rc::Rc;

use addr2line::fallible_iterator::FallibleIterator;
use addr2line::{gimli, object, Context};
use lazy_static::lazy_static;
use regex::{Captures, Regex};

use crate::prelude::*;

use super::cargo::Cargo;

mod x86_64;

pub struct Spec<'a> {
    /// Name of the crate that `binary` was built from
    pub crate_name: &'a str,
    /// Binary to run. Must have been built for `platform`
    pub binary: &'a Utf8Path,
    /// Platform to run QEMU for
    pub platform: Platform,
    /// Memory specification for the VM
    pub memory: &'a str,
    /// Number of CPUs for the VM
    pub cpus: usize,
    /// Include a debug exit device (per https://docs.rs/qemu-exit/latest/qemu_exit/)
    pub debug_exit: bool,
}

/// Creates a new QEMU command for `platform`, including any
/// platform-specific arguments.
fn command_for(platform: Platform) -> Command {
    match platform {
        Platform::X86_64 => {
            let mut command = Command::new("qemu-system-x86_64");
            command.args([
                // UEFI firmware
                "-drive",
                "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.fd",
                "-drive",
                "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_VARS.fd",
                // CPU type
                "-machine",
                "q35,accel=kvm",
            ]);
            command
        }
    }
}

pub struct Qemu {
    /// Cargo wrapper, used for platforms that require additional bootloader
    /// compilation
    cargo: Rc<Cargo>,
}

impl Qemu {
    pub fn new(cargo: Rc<Cargo>) -> Qemu {
        Qemu { cargo }
    }

    pub fn run(&self, spec: Spec) -> Result<ExitStatus> {
        let mut cmd = command_for(spec.platform);
        // TODO: fifo for serial console so monitor can use stdio
        cmd.args(["--no-reboot", "-serial", "stdio"]);
        cmd.args(["-m", spec.memory]);
        cmd.arg("-smp").arg(format!("cpus={}", spec.cpus));
        self.add_binary(&mut cmd, &spec)?;

        if spec.debug_exit {
            match spec.platform {
                Platform::X86_64 => {
                    cmd.args(["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"]);
                }
            }
        }

        cmd.stdout(Stdio::piped());
        log::debug!("QEMU command: {cmd:?}");

        let mut proc = cmd.spawn().wrap_err("could not start qemu")?;

        let filter = SymbolizeFilter::new(spec.binary)?;
        let stdout = io::stdout().lock();
        filter.drain(BufReader::new(proc.stdout.take().unwrap()), stdout)?;

        proc.wait().wrap_err("could not run qemu")
    }

    /// Configure QEMU to boot `spec.binary` via the platform-appropriate
    /// bootloader
    fn add_binary(&self, cmd: &mut Command, spec: &Spec) -> Result<()> {
        let boot_image = x86_64::build_boot_image(spec.crate_name, spec.binary, &self.cargo)?;
        cmd.arg("-drive")
            .arg(format!("format=raw,file={boot_image}"));
        Ok(())
    }
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
                write!(buf, " (inlined by) ")?;
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
