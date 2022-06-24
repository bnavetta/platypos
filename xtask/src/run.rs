use std::process::{Command, ExitStatus};

use clap::Args;
use color_eyre::eyre::{bail, Context};
use color_eyre::Result;

use crate::build::{build, BuiltKernel};
use crate::output::Output;
use crate::platform::Platform;

static UEFI_FIRMWARE_FILES: &[&str] = &[
    "/usr/share/ovmf/x64/OVMF_CODE.fd",
    "/usr/share/ovmf/x64/OVMF_VARS.fd",
];

#[derive(Debug, Args)]
pub struct RunOpts {
    #[clap(long, arg_enum, default_value_t = Platform::X86_64)]
    platform: Platform,
}

#[derive(Debug, Args)]
pub struct TestOpts {
    #[clap(long, arg_enum, default_value_t = Platform::X86_64)]
    platform: Platform,
}

impl RunOpts {
    pub fn exec(self, output: Output) -> Result<()> {
        let kernel = build(&output, self.platform)?;
        let status = run(&output, &kernel, &["-s"])?;
        if !status.success() {
            bail!("QEMU failed with {status}");
        }

        Ok(())
    }
}

fn run(_output: &Output, kernel: &BuiltKernel, qemu_options: &[&str]) -> Result<ExitStatus> {
    let mut qemu_cmd = Command::new(match kernel.platform {
        Platform::X86_64 => "qemu-system-x86_64",
    });

    qemu_cmd
        .arg("-drive")
        .arg(format!("format=raw,file={}", kernel.boot_image))
        .arg("--no-reboot")
        .args(["-m", "1G"])
        .args(["-serial", "stdio"])
        .args(qemu_options);

    if matches!(kernel.platform, Platform::X86_64) {
        // VM configuration
        qemu_cmd.args(["-machine", "q35,accel=kvm"]);

        // Add UEFI firmware arguments
        for file in UEFI_FIRMWARE_FILES.iter() {
            qemu_cmd
                .arg("-drive")
                .arg(format!("if=pflash,format=raw,readonly=on,file={file}"));
        }
    }

    qemu_cmd.status().wrap_err("Could not run qemu")
}
