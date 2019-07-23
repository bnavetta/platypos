use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use ansi_term::{Style, Color};
use cargo_metadata::{MetadataCommand, Metadata};
use exitfailure::ExitFailure;
use failure::{Error, bail};
use structopt::StructOpt;
use tmux_interface::{TmuxInterface, NewSession, NewWindow, SplitWindow};

use crate::ovmf::Ovmf;

mod cargo;
mod ovmf;

/// Style for printing different steps the runner performs
fn step_style() -> Style {
    Color::Cyan.bold()
}

#[derive(Debug, StructOpt)]
struct RunnerArgs {
    /// Run in debug mode. This will start the GDB server and wait for a connection
    #[structopt(short = "d", long = "debug")]
    debug: bool,

    /// QEMU log items to enable
    ///
    /// See qemu-system-x86_64 -d help for available items
    #[structopt(long = "qemu-log")]
    qemu_log: Option<String>,

    /// Amount of memory in MiB to allocate to the VM
    #[structopt(default_value = "256", long = "qemu-memory")]
    qemu_memory: usize,

    /// Show the serial console and QEMU monitor side-by-side instead of in separate TMUX windows
    #[structopt(short = "s", long = "split")]
    split: bool,

    #[structopt(parse(from_os_str))]
    kernel_executable: PathBuf,
}

#[paw::main]
fn main(args: RunnerArgs) -> Result<(), ExitFailure> {
    let workspace_metadata = MetadataCommand::new().no_deps().exec()?;

    let ovmf = Ovmf::create(&workspace_metadata)?;

    println!("{}", step_style().paint("Building UEFI loader"));
    let loader_executable = cargo::build_package(&workspace_metadata, "platypos_loader", "x86_64-unknown-uefi")?;

    let system_partition = build_system_partition(&workspace_metadata, &loader_executable, &args.kernel_executable)?;

    run(&workspace_metadata, &ovmf, &system_partition, &args)?;

    Ok(())
}


fn build_system_partition(metadata: &Metadata, loader_executable: &PathBuf, kernel_executable: &PathBuf) -> Result<PathBuf, Error> {
    let esp_dir = metadata.target_directory.join("esp");

    // Ensure we're starting with a clean slate
    if esp_dir.exists() {
        fs::remove_dir_all(&esp_dir)?;
    }
    fs::create_dir_all(&esp_dir)?;

    let boot_dir = esp_dir.join("EFI/Boot");
    fs::create_dir_all(&boot_dir)?;
    fs::copy(&loader_executable, boot_dir.join("BootX64.efi"))?;

    fs::copy(&kernel_executable, esp_dir.join("platypos_kernel"))?;

    Ok(esp_dir)
}

fn check_exists(command: &str) -> bool {
    match Command::new(command).spawn() {
        Ok(mut child) => {
            // just trying to see if it exists
            let _ = child.kill();
            true
        },
        Err(e) => e.kind() != ErrorKind::NotFound
    }
}

fn run(metadata: &Metadata, ovmf: &Ovmf, system_partition: &PathBuf, args: &RunnerArgs) -> Result<(), Error> {
    // Fail fast if any necessary commands are missing
    if !check_exists("qemu-system-x86_64") {
        bail!("qemu-system-x86_64 not found");
    }

    if !check_exists("tmux") {
        bail!("tmux not found");
    }

    if !check_exists("socat") {
        bail!("socat not found");
    }

    let monitor_socket = metadata.target_directory.join("qemu-monitor.sock");
    if monitor_socket.exists() {
        fs::remove_file(&monitor_socket)?;
    }

    let mut qemu_args = vec![
        "qemu-system-x86_64".to_string(),

        // Machine settings - use a recent CPU with hardware acceleration if available
        // TODO: see if hvf works out for macOS acceleration
        "-machine".to_string(), "q35,accel=kvm:tcg".to_string(),

        // OVMF - attach the two firmware files
        "-drive".to_string(), format!("if=pflash,format=raw,file={},readonly=on", ovmf.firmware().display()),
        "-drive".to_string(), format!("if=pflash,format=raw,file={},readonly=on", ovmf.vars_template().display()),

        // Emulated EFI system partition
        "-drive".to_string(), format!("format=raw,file=fat:rw:{}", system_partition.display()),

        // Map the QEMU exit signal to port 0xf4
        "-device".to_string(), "isa-debug-exit,iobase=0xf4,iosize=0x04".to_string(),

        // Redirect the serial port to stdout
        // OVMF will also redirect UEFI stdout to this
        "-serial".to_string(), "stdio".to_string(),

        // Run the QEMU monitor on a Unix domain socket, so another tmux window can connect to it
        "-monitor".to_string(),  format!("unix:{},server,nowait", monitor_socket.display()),

        "-m".to_string(), args.qemu_memory.to_string()
    ];

    if let Some(log_items) = &args.qemu_log {
        qemu_args.push("-d".to_string());
        qemu_args.push(log_items.to_string());
    }

    if args.debug {
        qemu_args.push("-s".to_string()); // start GDB debug server
        qemu_args.push("-S".to_string()); // wait for a connection from a debugger or the monitor
    }

    let qemu_cmd = qemu_args.join(" ");

    let mut tmux = TmuxInterface::new();
    tmux.colours256 = Some(true);

    // Kill any leftover tmux sessions
    tmux.kill_session(None, None, Some("platypos-runner"))?;

    tmux.new_session(&NewSession {
        detached: Some(true),
        session_name: Some("platypos-runner"),
        window_name: Some("qemu"),
        shell_command: Some(&qemu_cmd),
        ..Default::default()
    })?;

    // Wait for QEMU to start
    // socat will fail if the socket file doesn't exist
    let wait_start = Instant::now();
    while !monitor_socket.exists() {
        thread::sleep(Duration::from_millis(500));

        if Instant::now() - wait_start >= Duration::from_secs(30) {
            bail!("Timed out waiting for QEMU to start");
        }
    }

    let monitor_command = format!("socat - unix-connect:{}", monitor_socket.display());

    if args.split {
        tmux.split_window(&SplitWindow {
            horizontal: Some(true),
            shell_command: Some(&monitor_command),
            ..Default::default()
        })?;
    } else {
        tmux.new_window(NewWindow {
            window_name: Some("qemu-monitor"),
            shell_command: Some(&monitor_command),
            detached: Some(true),
            ..Default::default()
        })?;
    }

    // Run this directly so it inherits stdin/stdout/stderr
    let mut attach = Command::new("tmux")
        .args(&["attach-session", "-t", "platypos-runner"])
        .spawn()?;

    attach.wait()?;

    // Clean up after ourselves
    tmux.kill_session(None, None, Some("platypos-runner"))?;

    // it seems like QEMU removes the socket on shutdown, at least if it exits cleanly
    if monitor_socket.exists() {
        fs::remove_file(&monitor_socket)?;
    }


    Ok(())
}