use clap::{ArgEnum, Args};
use color_eyre::eyre::eyre;
use color_eyre::Result;
use supports_color::Stream;

#[derive(Debug, Args)]
pub struct OutputOpts {
    #[clap(long, arg_enum, global = true, default_value_t = Color::Auto)]
    color: Color,

    #[clap(long, short, global = true)]
    verbose: bool,
}

#[derive(ArgEnum, Clone, Copy, Debug)]
enum Color {
    Always,
    Auto,
    Never,
}

pub struct Output {
    pub verbose: bool,
    color: Color,
}

impl OutputOpts {
    pub(crate) fn init(self) -> Result<Output> {
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

        Ok(Output {
            verbose: self.verbose,
            color: self.color,
        })
    }
}

impl Output {
    pub fn should_colorize(&self, stream: Stream) -> bool {
        match self.color {
            Color::Always => true,
            Color::Auto => supports_color::on_cached(stream)
                .map(|l| l.has_basic)
                .unwrap_or(false),
            Color::Never => false,
        }
    }
}
