use clap::Parser;

mod build;
mod cargo;
mod command;
mod output;
mod platform;
mod run;

use command::XTask;

fn main() -> color_eyre::Result<()> {
    let app = XTask::parse();
    app.exec()
}
