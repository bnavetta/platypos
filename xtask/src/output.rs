use clap::{Args, ValueEnum};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use log::{LevelFilter, Log};
use owo_colors::OwoColorize;
use supports_color::Stream;

#[derive(Debug, Args)]
pub struct OutputOpts {
    #[arg(long, value_enum, global = true, default_value_t = Color::Auto)]
    color: Color,

    #[arg(long, short, global = true)]
    verbose: bool,
}

#[derive(ValueEnum, Clone, Copy, Debug)]
enum Color {
    Always,
    Auto,
    Never,
}

/// Very simple logger to respect command-line color/verbosity preferences
struct OutputLog {
    filter: LevelFilter,
}

impl OutputOpts {
    pub(crate) fn init(self) -> Result<()> {
        color_eyre::install()?;

        match self.color {
            Color::Always => {
                owo_colors::set_override(true);
            }
            Color::Auto => {
                owo_colors::unset_override();
            }
            Color::Never => {
                owo_colors::set_override(false);
            }
        }

        if matches!(self.color, Color::Always | Color::Auto) {
            enable_ansi_support::enable_ansi_support()
                .map_err(|e| eyre!("could not enable ANSI colors: code {e}"))?;
        }

        let level_filter = if self.verbose {
            log::LevelFilter::Trace
        } else {
            log::LevelFilter::Info
        };
        log::set_boxed_logger(Box::new(OutputLog {
            filter: level_filter,
        }))?;
        log::set_max_level(level_filter);

        Ok(())
    }
}

impl Log for OutputLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.filter
    }

    fn log(&self, record: &log::Record) {
        if self.enabled(record.metadata()) {
            match record.level() {
                log::Level::Error => {
                    print!("{} ", "🚨".if_supports_color(Stream::Stdout, |c| c.red()))
                }
                log::Level::Warn => {
                    print!("{}", "⚠️".if_supports_color(Stream::Stdout, |c| c.yellow()))
                }
                _ => (),
            }

            println!("{}", record.args())
        }
    }

    fn flush(&self) {}
}
