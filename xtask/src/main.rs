use clap::Parser;

mod command;
mod output;
mod platform;
mod prelude;
mod tools;

use command::XTask;

fn main() -> color_eyre::Result<()> {
    let app = XTask::parse();
    app.exec()
}
