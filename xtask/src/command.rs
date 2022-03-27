use clap::{Parser, Subcommand};
use color_eyre::Result;

use crate::build::BuildOpts;
use crate::output::OutputOpts;
use crate::run::RunOpts;

#[derive(Debug, Parser)]
pub struct XTask {
    #[clap(flatten)]
    output: OutputOpts,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Build(BuildOpts),
    Run(RunOpts),
}

impl XTask {
    pub fn exec(self) -> Result<()> {
        let output = self.output.init()?;
        match self.command {
            Command::Build(app) => app.exec(output),
            Command::Run(app) => app.exec(output),
        }
    }
}
