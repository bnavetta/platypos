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

    /// defmt logging filter
    #[clap(long, default_value = "trace")]
    defmt: String,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build,
    Run(QemuOpts),
    Test(QemuOpts),
}

#[derive(Debug, Args)]
struct QemuOpts {
    /// Number of CPUs for the QEMU VM
    #[clap(long, default_value = "1")]
    cpus: u8,

    /// Memory for the QEMU VM
    #[clap(long, default_value = "1G")]
    memory: String,
}

struct Context {
    platform: Platform,
    cargo: Rc<Cargo>,
    qemu: Qemu,
    defmt_filter: String,
}

const KERNEL_CRATE: &str = "platypos_kernel";

impl XTask {
    pub fn exec(self) -> Result<()> {
        self.output.init()?;

        let context = Context::new(self.tools.platform, self.tools.cargo, self.tools.defmt);

        match self.command {
            Command::Build => do_build(&context),
            Command::Run(opts) => do_run(&context, opts),
            Command::Test(opts) => do_test(&context, opts),
        }
    }
}

impl Context {
    fn new(
        platform: Platform,
        cargo_override: Option<Utf8PathBuf>,
        defmt_filter: String,
    ) -> Context {
        let cargo = Rc::new(Cargo::new(cargo_override));
        let qemu = Qemu::new(cargo.clone());
        Context {
            platform,
            cargo,
            qemu,
            defmt_filter,
        }
    }

    fn build(&self, crate_name: &str) -> Result<Utf8PathBuf> {
        let output = self.cargo.build(&cargo::BuildSpec {
            crate_name,
            platform: self.platform,
            test: false,
            defmt_filter: &self.defmt_filter,
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
            debug_exit: false,
        })
    }
}

fn do_build(context: &Context) -> Result<()> {
    context.build(KERNEL_CRATE)?;
    Ok(())
}

fn do_run(context: &Context, opts: QemuOpts) -> Result<()> {
    let status = context.run(KERNEL_CRATE, &opts.memory, opts.cpus.into())?;

    if !status.success() {
        Err(eyre!("QEMU failed: {status}"))
    } else {
        Ok(())
    }
}

fn do_test(context: &Context, opts: QemuOpts) -> Result<()> {
    let output = context.cargo.build(&cargo::BuildSpec {
        crate_name: KERNEL_CRATE,
        platform: context.platform,
        test: true,
        defmt_filter: &context.defmt_filter,
    })?;
    let test_kernel = output.executable(KERNEL_CRATE)?;

    let status = context.qemu.run(qemu::Spec {
        crate_name: KERNEL_CRATE,
        binary: test_kernel,
        platform: context.platform,
        memory: &opts.memory,
        cpus: opts.cpus.into(),
        debug_exit: true,
    })?;

    match status.code() {
        Some(code) => {
            // Match the success code set in ktest/src/lib.rs - QEMU's debug exit device
            // can't exit with 0
            if code != 3 {
                bail!("Tests failed")
            }
        }
        None => bail!("QEMU killed by signal: {status}"),
    }

    Ok(())
}
