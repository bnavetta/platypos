//! pos_cli - command-line tool for building and running PlatypOS
//!
//! This is 1000% more complicated than necessary, but I have to deal with too many sad bash scripts at work so...

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use eyre::{Result, WrapErr, Report, eyre};
use log::{debug, info};
use structopt::StructOpt;

mod qemu;

#[derive(Debug, StructOpt)]
#[structopt(name = "pos", about = "PlatypOS build tool")]
struct PosArgs {
    /// Platform to build for
    #[structopt(long, default_value = "x86_64")]
    platform: Platform,

    /// Build mode
    #[structopt(long, default_value = "debug")]
    mode: BuildMode,

    #[structopt(flatten)]
    verbosity: clap_verbosity_flag::Verbosity,

    #[structopt(subcommand)]
    task: Task,
}

#[derive(StructOpt, Debug, Copy, Clone, Eq, PartialEq)]
enum Task {
    /// Build PlatypOS
    Build,
    /// Run PlatypOS in QEMU
    Run {
        /// Debug with a GDB server
        #[structopt(long)]
        debug: bool,
    },
    /// connect to a running QEMU GDB server
    Debugger,
}

fn main() -> Result<()> {
    let args: PosArgs = PosArgs::from_args();
    let mut verbosity = args.verbosity.clone();
    verbosity.set_default(Some(log::Level::Info));
    if let Some(level) = verbosity.log_level() {
        pretty_env_logger::formatted_builder()
            .filter(None, level.to_level_filter())
            .init();
    }

    match args.task {
        Task::Build => {
            build_kernel(args.platform, args.mode).wrap_err("Building the kernel failed")?;
        },
        Task::Run { debug } => {
            match args.platform {
                Platform::X86_64 => {
                    let loader = build_uefi_loader(args.mode).wrap_err("Building the bootloader failed")?;
                    build_kernel(args.platform, args.mode).wrap_err("Building the kernel failed")?;
                    let kernel = kernel_path(args.platform, args.mode);
                    qemu::run_uefi(&loader, &kernel, debug)?;
                },
            }
        },
        Task::Debugger => {
            let kernel = kernel_path(args.platform, args.mode);
            qemu::connect_debugger(&kernel)?;
        }
    }

    Ok(())
}

/// PlatypOS platform to build/launch
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum Platform {
    X86_64,
}

impl Platform {
    fn name(self) -> &'static str {
        match self {
            Platform::X86_64 => "x86_64"
        }
    }

    /// Directory containing the platform-specific entrypoint crate.
    fn crate_dir(self) -> PathBuf {
        PathBuf::from(format!("kernel_{}", self.name()))
    }

    fn crate_name(self) -> String {
        format!("platypos_kernel_{}", self.name())
    }

    /// Name of the Rust target corresponding to this platform.
    fn target_name(self) -> String {
        format!("{}-os", self.name())
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl FromStr for Platform {
    type Err = Report;

    fn from_str(s: &str) -> Result<Platform, Report> {
        match s {
            "x86_64" => Ok(Platform::X86_64),
            _ => Err(eyre!("Unknown platform: `{}`", s))
        }
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
enum BuildMode {
    Debug,
    Release
}

impl FromStr for BuildMode {
    type Err = Report;

    fn from_str(s: &str) -> Result<BuildMode, Report> {
        match s {
            "debug" => Ok(BuildMode::Debug),
            "release" => Ok(BuildMode::Release),
            _ => Err(eyre!("Cannot build for `{}`", s))
        }
    }
}

impl BuildMode {
    /// Subdirectory of `target/<target>` with artifacts built in this mode.
    fn target_dir(self) -> &'static Path {
        match self {
            BuildMode::Debug => Path::new("debug"),
            BuildMode::Release => Path::new("release")
        }
    }

    /// Adds cargo build flags
    fn add_flags(self, command: &mut Command) {
        match self {
            BuildMode::Debug => (),
            BuildMode::Release => { command.arg("--release"); }
        }
    }
}

/// Builds the x86-64 UEFI loader, returning a path to the built executable
fn build_uefi_loader(mode: BuildMode) -> Result<PathBuf> {
    info!("Building x86-64 UEFI loader...");
    let mut command = Command::new("cargo");
    command.arg("build")
        .current_dir("loader_uefi");
    mode.add_flags(&mut command);
    run(&mut command)?;
    let mut loader_path = target_dir("x86_64-unknown-uefi", mode);
    loader_path.push("platypos_loader_uefi.efi");
    Ok(loader_path)
}

/// Builds the kernel, returning a path to the built executable
fn build_kernel(platform: Platform, mode: BuildMode) -> Result<()> {
    info!("Building kernel for {}", platform);
    let mut cargo = Command::new("cargo");
    cargo.arg("build")
        .current_dir(platform.crate_dir());
    mode.add_flags(&mut cargo);
    run(&mut cargo)?;
    Ok(())
}

/// Path to the kernel executable
fn kernel_path(platform: Platform, mode: BuildMode) -> PathBuf {
    let mut kernel_path = target_dir(platform.target_name(), mode);
    kernel_path.push(platform.crate_name());
    kernel_path
}

fn target_dir<P: AsRef<Path>>(target: P, mode: BuildMode) -> PathBuf {
    let mut path = PathBuf::from("target");
    path.push(target);
    path.push(mode.target_dir());
    path
}

fn run(command: &mut Command) -> Result<()> {
    debug!("Running {:?}", command);
    let mut child = command.spawn()
        .wrap_err("Failed to start process")?;
    let status = child.wait()
        .wrap_err("Failed waiting for process to finish")?;
    debug!("{:?} exited with {}", command, status);
    if status.success() {
        Ok(())
    } else {
        Err(eyre!("{:?} failed", command))
    }
}