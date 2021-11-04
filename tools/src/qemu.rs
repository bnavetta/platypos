use std::process::Command;

use miette::{miette, Context, IntoDiagnostic, Result};
use owo_colors::OwoColorize;

use crate::build::BuildInfo;

/// Path to the OpenSBI firmware that ships with QEMU's extras package
const OPENSBI_PATH: &str = "/usr/share/qemu/opensbi-riscv64-generic-fw_dynamic.elf";

pub fn run(build_info: &BuildInfo) -> Result<()> {
    let cmd = prepare_qemu_command(build_info);
    launch_qemu(cmd)
}

pub fn debug(build_info: &BuildInfo) -> Result<()> {
    let mut cmd = prepare_qemu_command(build_info);
    // Start a GDB server and wait for a debugger to connect before running the OS
    cmd.args(&["-s", "-S"]);
    launch_qemu(cmd)
}

fn prepare_qemu_command(build_info: &BuildInfo) -> Command {
    let mut cmd = Command::new("qemu-system-riscv64");
    cmd.args(&[
        "-machine",
        "virt",
        "-cpu",
        "rv64",
        // Configure with 4 CPUs and 1GB of RAM
        "-smp",
        "4",
        "-m",
        "1G",
        // Add VirtIO devices
        "-device",
        "virtio-rng-device",
        "-device",
        "virtio-gpu-device",
        "-device",
        "virtio-net-device",
        "-device",
        "virtio-tablet-device",
        "-device",
        "virtio-keyboard-device",
        "-nographic",
        "-serial",
        "mon:stdio",
        "-bios",
        OPENSBI_PATH,
    ]);
    cmd.arg("-kernel").arg(&build_info.kernel_binary);
    cmd
}

fn launch_qemu(mut cmd: Command) -> Result<()> {
    println!("Running {:?}", cmd.green());

    let mut child = cmd
        .spawn()
        .into_diagnostic()
        .wrap_err("Starting QEMU failed")?;

    let status = child
        .wait()
        .into_diagnostic()
        .wrap_err("Waiting for QEMU failed")?;

    if !status.success() {
        Err(miette!("QEMU exited with {}", status))
    } else {
        Ok(())
    }
}
