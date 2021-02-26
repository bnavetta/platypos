use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::Command;
use std::os::unix::fs as unix_fs;
use std::os::unix::process::CommandExt;

use eyre::{Result, WrapErr, eyre};
use log::{info, debug};
use tempfile::{TempDir, NamedTempFile};

use crate::run;

/// Runs PlatypOS in QEMU using an UEFI bootloader
pub fn run_uefi(uefi_app: &Path, kernel_executable: &Path, debug: bool) -> Result<()> {
    info!("Running in QEMU");
    debug!("Kernel: {}", kernel_executable.display());
    debug!("Bootloader: {}", uefi_app.display());
    let esp_root = build_esp(uefi_app, kernel_executable)?;

    let mut qemu = Command::new("qemu-system-x86_64");
    qemu
        .args(&["-machine", "q35,accel=kvm:tcg"])
        // OVMF (EFI firmware)
        .args(&[
            "-drive", "if=pflash,format=raw,file=/usr/share/ovmf/x64/OVMF_CODE.fd,readonly=on",
            "-drive", "if=pflash,format=raw,file=/usr/share/ovmf/x64/OVMF_VARS.fd,readonly=on"
        ])
        // EFI system partition as a FAT drive
        .arg("-drive").arg(format!("format=raw,file=fat:rw:{}", esp_root.path().display()))
        // Use port 0xf4 to exit QEMU
        .args(&["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04"])
        // Redirect serial port and UEFI stdout to a log file
        .args(&["-serial", "file:target/qemu.log"])
        // Start the QEMU monitor on stdin/out
        .args(&["-monitor", "stdio"])
        // Amount of memory, in MiB
        .args(&["-m", "1024"])
        // Additional logging, see qemu-system-x86_64 -d help
        .args(&["-d", "int,guest_errors,in_asm"]);
    if debug {
        qemu.args(&["-s", "-S"]);
    }

    run(&mut qemu)?;

    Ok(())
}

/// Connects to the GDB server
pub fn connect_debugger(kernel_path: &Path) -> Result<()> {
    println!("Remember, use hbreak instead of break for best results!");

    let mut init = NamedTempFile::new()
        .wrap_err("Could not create GDB command file")?;
    writeln!(&mut init, "symbol-file {}", kernel_path.display())
        .wrap_err("Could not write to GDB command file")?;
    writeln!(&mut init, "target remote localhost:1234")
        .wrap_err("Could not write to GDB command file")?;
    let init_path = init.into_temp_path();

    let mut gdb = Command::new("gdb");
    gdb.arg("-ix").arg(&init_path);
    // exec GDB instead of spawning and waiting for it. This makes sure it receives Ctrl-C properly
    // instead of exiting the wrapper program. Alternatively, use tcsetpgrp
    gdb.exec();
    Err(eyre!("Could not execute GDB"))
}

/// Creates the EFI system partition directory
fn build_esp(uefi_app: &Path, kernel_executable: &Path) -> Result<TempDir> {
    debug!("Building EFI system partition");
    let esp_root = TempDir::new()
        .wrap_err("Could not create temporary directory for EFI system partition")?;
    debug!("ESP root: {}", esp_root.path().display());

    let uefi_app = fs::canonicalize(uefi_app)
        .wrap_err("Could not get absolute path to UEFI bootloader")?;
    let kernel_executable = fs::canonicalize(kernel_executable)
        .wrap_err("Could not get absolute path to kernel")?;

    let boot_dir = esp_root.path().join("EFI").join("Boot");
    fs::create_dir_all(&boot_dir)
        .wrap_err("Could not create EFI/Boot directory in ESP")?;
    unix_fs::symlink(uefi_app, boot_dir.join("Bootx64.efi"))
        .wrap_err("Could not link UEFI bootloader application into ESP")?;
    unix_fs::symlink(kernel_executable, esp_root.path().join("platypos_kernel"))
        .wrap_err("Could not link kernel into ESP")?;

    Ok(esp_root)
}
