use std::process::ExitStatus;
use std::rc::Rc;

use clap::{Args, Parser, Subcommand};

use crate::output::OutputOpts;
use crate::tools::cargo::{self, Cargo};

use crate::prelude::*;
use crate::tools::qemu::{self, Qemu};

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

struct Context {
    platform: Platform,
    cargo: Rc<Cargo>,
    qemu: Qemu,
}

const KERNEL_CRATE: &str = "platypos_kernel";

impl XTask {
    pub fn exec(self) -> Result<()> {
        self.output.init()?;

        let context = Context::new(self.tools.platform, self.tools.cargo);

        match self.command {
            Command::Build => do_build(&context),
            Command::Run => do_run(&context),
        }
    }
}

impl Context {
    fn new(platform: Platform, cargo_override: Option<Utf8PathBuf>) -> Context {
        let cargo = Rc::new(Cargo::new(cargo_override));
        let qemu = Qemu::new(cargo.clone());
        Context {
            platform,
            cargo,
            qemu,
        }
    }

    fn build(&self, crate_name: &str) -> Result<Utf8PathBuf> {
        let output = self.cargo.build(&cargo::BuildSpec {
            crate_name,
            platform: self.platform,
        })?;
        let binary = output.executable(crate_name)?;
        log::info!(
            "Built {} at {}",
            crate_name.if_supports_color(Stream::Stdout, |c| c.green()),
            binary.if_supports_color(Stream::Stdout, |c| c.magenta())
        );
        Ok(binary.to_owned())
    }

    fn run(&self, crate_name: &str, memory: &str, cpus: usize) -> Result<ExitStatus> {
        let binary = self.build(crate_name)?;
        self.qemu.run(qemu::Spec {
            crate_name,
            binary: &binary,
            platform: self.platform,
            memory,
            cpus,
        })
    }
}

fn do_build(context: &Context) -> Result<()> {
    context.build(KERNEL_CRATE)?;
    Ok(())
}

fn do_run(context: &Context) -> Result<()> {
    let status = context.run(KERNEL_CRATE, "1G", 1)?;

    if !status.success() {
        Err(eyre!("QEMU failed: {status}"))
    } else {
        Ok(())
    }
}
