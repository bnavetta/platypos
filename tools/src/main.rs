use std::path::Path;

use argh::FromArgs;
use miette::{miette, Result};

mod build;
mod gdb;
mod qemu;

/// Developer tools for PlatypOS
#[derive(FromArgs, PartialEq, Debug)]
struct Args {
    /// build in release mode
    #[argh(switch)]
    release: bool,

    #[argh(subcommand)]
    action: Action,
}

#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand)]
enum Action {
    Build(BuildAction),
    Run(RunAction),
    Debug(DebugAction),
    Debugger(DebuggerAction),
}

/// Build PlatypOS
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "build")]
struct BuildAction {}

/// Run PlatypOS in QEMU
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "run")]
struct RunAction {}

/// Run PlatypOS in QEMU with a debug server
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "debug")]
struct DebugAction {}

/// Run GDB attached to an already-running QEMU instance
#[derive(FromArgs, PartialEq, Debug)]
#[argh(subcommand, name = "debugger")]
struct DebuggerAction {}

fn main() -> Result<()> {
    let args: Args = argh::from_env();

    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .ok_or_else(|| miette!("Could not find project root"))?;

    let build_mode = if args.release {
        build::Mode::Release
    } else {
        build::Mode::Debug
    };

    match args.action {
        Action::Build(_build) => {
            build::build_kernel(root, build_mode)?;
        }
        Action::Run(_) => {
            let build_info = build::build_kernel(root, build_mode)?;
            qemu::run(&build_info)?;
        }
        Action::Debug(_) => {
            let build_info = build::build_kernel(root, build_mode)?;
            qemu::debug(&build_info)?;
        }
        Action::Debugger(_) => {
            let build_info = build::build_kernel(root, build_mode)?;
            gdb::debugger(root, &build_info)?;
        }
    }

    Ok(())
}
