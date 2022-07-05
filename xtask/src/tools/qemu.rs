//! Wrapper around QEMU

use std::ffi::OsString;
use std::io::{self, BufReader};
use std::process::ExitStatus;
use std::rc::Rc;

use crate::prelude::*;
use crate::tools::qemu::decoder::Decoder;

use super::cargo::Cargo;

mod decoder;
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
fn command_for(platform: Platform) -> (&'static str, Vec<OsString>) {
    match platform {
        Platform::X86_64 => {
            let args: Vec<OsString> = [
                // UEFI firmware
                "-drive",
                "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_CODE.fd",
                "-drive",
                "if=pflash,format=raw,readonly=on,file=/usr/share/ovmf/x64/OVMF_VARS.fd",
                // CPU type
                "-machine",
                "q35,accel=kvm",
            ]
            .map(Into::into)
            .into();
            ("qemu-system-x86_64", args)
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
        let (exe, mut args) = command_for(spec.platform);
        // TODO: fifo for serial console so monitor can use stdio
        args.extend(["--no-reboot", "-serial", "stdio", "-m", spec.memory].map(Into::into));
        args.push("-smp".into());
        args.push(format!("cpus={}", spec.cpus).into());
        self.add_binary(&mut args, &spec)?;

        if spec.debug_exit {
            match spec.platform {
                Platform::X86_64 => {
                    args.extend(
                        ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"].map(Into::into),
                    );
                }
            }
        }

        let cmd = duct::cmd(exe, args).unchecked();

        log::debug!("QEMU command: {cmd:?}");

        // ReaderHandle will kill QEMU if it's dropped due to an error
        let mut output = cmd.reader().wrap_err("could not start qemu")?;

        // let filter = SymbolizeFilter::new(spec.binary)?;
        let stdout = io::stdout().lock();
        let decoder = Decoder::new(spec.platform, spec.binary)?;
        decoder.decode(BufReader::new(&mut output), stdout)?;
        // filter.drain(BufReader::new(proc.stdout.take().unwrap()), stdout)?;

        // Guaranteed that if the reader completed, this will return Ok(Some(_))
        Ok(output.try_wait().unwrap().unwrap().status)
    }

    /// Configure QEMU to boot `spec.binary` via the platform-appropriate
    /// bootloader
    fn add_binary(&self, args: &mut Vec<OsString>, spec: &Spec) -> Result<()> {
        let boot_image = x86_64::build_boot_image(spec.crate_name, spec.binary, &self.cargo)?;
        args.push("-drive".into());
        args.push(format!("format=raw,file={boot_image}").into());
        Ok(())
    }
}
