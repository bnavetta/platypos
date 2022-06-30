use clap::Parser;

mod tools;

mod command;
mod output;
mod platform;
mod runner;

use command::XTask;

fn main() -> color_eyre::Result<()> {
    let app = XTask::parse();
    app.exec()
}
