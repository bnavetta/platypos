use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Command, Stdio};
use std::{env, fmt};

use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::Message;
use color_eyre::eyre::{bail, eyre, Context};
use color_eyre::Result;
use owo_colors::{OwoColorize, Stream};

use crate::output::Output;
use crate::platform::Platform;

pub struct Cargo {
    pub cargo_bin: Utf8PathBuf,
}

pub struct BuildOutputs {
    pub executables: HashMap<String, Utf8PathBuf>,
}

impl Cargo {
    pub fn new() -> Cargo {
        let cargo_bin = env::var("CARGO")
            .map(Utf8PathBuf::from)
            .unwrap_or_else(|_| Utf8PathBuf::from("cargo"));
        Cargo { cargo_bin }
    }

    pub fn build(
        &self,
        output: &Output,
        platform: Platform,
        crate_name: &str,
    ) -> Result<BuildOutputs> {
        let mut command = Command::new(&self.cargo_bin);
        command
            .args(&[
                "build",
                "-p",
                crate_name,
                "--message-format=json-render-diagnostics",
                "--target",
                platform.target().as_str(),
            ])
            .args(platform.build_flags())
            .stdout(Stdio::piped());

        if output.verbose {
            println!(
                "Building {} for {}",
                crate_name.if_supports_color(Stream::Stdout, |c| c.green()),
                platform.if_supports_color(Stream::Stdout, |c| c.green())
            );
        }

        let mut command = command.spawn().wrap_err("could not exec Cargo")?;

        let mut outputs = BuildOutputs {
            executables: HashMap::new(),
        };

        let reader = BufReader::new(command.stdout.take().unwrap());
        for message in Message::parse_stream(reader) {
            let message = message.wrap_err("could not parse Cargo message")?;
            if let Message::CompilerArtifact(artifact) = message {
                if let Some(executable) = artifact.executable {
                    outputs.executables.insert(artifact.target.name, executable);
                }
            }
        }

        let status = command.wait().wrap_err("could not wait on Cargo")?;
        if status.success() {
            Ok(outputs)
        } else {
            bail!("cargo failed with {status}")
        }
    }
}

impl BuildOutputs {
    pub fn executable<'a>(&'a self, crate_name: &str) -> Result<&'a Utf8Path> {
        self.executables
            .get(crate_name)
            .map(|p| p.as_path())
            .ok_or_else(|| eyre!("no executable for crate {crate_name}"))
    }
}

impl fmt::Display for BuildOutputs {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("executables:\n")?;
        for (name, path) in self.executables.iter() {
            writeln!(f, "{name} = {path}")?;
        }
        Ok(())
    }
}
