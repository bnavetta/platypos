use camino::Utf8PathBuf;
use clap::{Args, Parser, Subcommand};
use color_eyre::eyre::eyre;
use color_eyre::Result;

use crate::output::OutputOpts;
use crate::platform::Platform;
use crate::runner;
use crate::tools::cargo::{self, Cargo};

#[derive(Debug, Parser)]
pub struct XTask {
    #[clap(flatten)]
    output: OutputOpts,

    #[clap(flatten)]
    tools: ToolOpts,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Args)]
struct ToolOpts {
    #[clap(long, global = true, env = "CARGO")]
    cargo: Option<Utf8PathBuf>,

    #[clap(long, arg_enum, default_value_t = Platform::X86_64)]
    platform: Platform,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build,
    Run,
}

const KERNEL_CRATE: &str = "platypos_kernel";

impl XTask {
    pub fn exec(self) -> Result<()> {
        self.output.init()?;

        let cargo = Cargo::new(self.tools.cargo);

        match self.command {
            Command::Build => do_build(self.tools.platform, &cargo),
            Command::Run => do_run(self.tools.platform, &cargo),
        }
    }
}

fn do_build(platform: Platform, cargo: &Cargo) -> Result<()> {
    build_kernel(platform, cargo)?;
    Ok(())
}

fn do_run(platform: Platform, cargo: &Cargo) -> Result<()> {
    let kernel = build_kernel(platform, cargo)?;

    let status = runner::run(KERNEL_CRATE, &kernel, cargo, platform)?;
    if !status.success() {
        Err(eyre!("QEMU failed: {status}"))
    } else {
        Ok(())
    }
}

fn build_kernel(platform: Platform, cargo: &Cargo) -> Result<Utf8PathBuf> {
    let output = cargo.build(&cargo::BuildSpec {
        crate_name: KERNEL_CRATE,
        platform,
    })?;

    let binary = output.executable(KERNEL_CRATE)?;
    log::info!("Built kernel at {}", binary);
    Ok(binary.to_owned())
}
