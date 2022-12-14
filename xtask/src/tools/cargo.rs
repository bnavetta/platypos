//! Wrapper for running Cargo

use std::collections::HashMap;
use std::io::BufReader;
use std::process::{Command, Stdio};

use cargo_metadata::Message;

use crate::prelude::*;

pub struct Cargo {
    cargo: Utf8PathBuf,
}

pub struct BuildSpec<'a> {
    /// Name of the crate to compile
    pub crate_name: &'a str,
    /// Platform to build for
    pub platform: Platform,
    /// Build as a test binary
    pub test: bool,
    pub defmt_filter: &'a str,
}

pub struct BuildOutput {
    /// Mapping from crate names to the built executables
    pub executables: HashMap<String, Utf8PathBuf>,
}

/// Contains flags for compilation
struct Flags {
    pub target_triple: String,
    /// Flags for `cargo build`
    pub build_flags: Vec<String>,
    /// Flags for rustc
    pub rust_flags: Vec<String>,
    /// Flags for the C++ compiler
    pub cxx_flags: Vec<String>,
}

/// Locates `Cargo.toml` for a crate in the PlatypOS workspace
pub fn manifest_path(crate_name: &str) -> Utf8PathBuf {
    match crate_name.strip_prefix("platypos_") {
        Some(n) => Utf8PathBuf::from(n),
        None => Utf8PathBuf::from(crate_name),
    }
    .join("Cargo.toml")
}

impl Cargo {
    pub fn new(cargo_override: Option<Utf8PathBuf>) -> Cargo {
        Cargo {
            cargo: cargo_override.unwrap_or_else(|| Utf8PathBuf::from("cargo")),
        }
    }

    /// Command template for running a cargo command
    pub fn command(&self) -> Command {
        Command::new(&self.cargo)
    }

    pub fn build(&self, spec: &BuildSpec) -> Result<BuildOutput> {
        log::info!(
            "Building {} for {}",
            spec.crate_name
                .if_supports_color(Stream::Stdout, |c| c.green()),
            spec.platform
                .if_supports_color(Stream::Stdout, |c| c.blue())
        );
        let flags = self.flags_for(spec.platform);

        let mut cmd = Command::new(&self.cargo);
        cmd.args(&[
            "build",
            "-p",
            spec.crate_name,
            "--message-format=json-render-diagnostics",
            "--target",
            &flags.target_triple,
        ])
        .args(flags.build_flags)
        .stdout(Stdio::piped());

        if spec.test {
            cmd.arg("--tests");
        }

        if !flags.rust_flags.is_empty() {
            let f = flags.rust_flags.join(" ");
            log::debug!("RUSTFLAGS = {f}");
            cmd.env("RUSTFLAGS", f);
        }

        if !flags.cxx_flags.is_empty() {
            let f = flags.cxx_flags.join(" ");
            log::debug!("CXXFLAGS = {f}");
            cmd.env("CXXFLAGS", f);
        }

        cmd.env("DEFMT_LOG", spec.defmt_filter);

        log::debug!("Cargo command line: {cmd:?}");

        let mut proc = cmd.spawn().wrap_err("cargo execution failed")?;

        let mut executables = HashMap::new();

        let reader = BufReader::new(proc.stdout.take().unwrap());
        for message in Message::parse_stream(reader) {
            let message = message.wrap_err("unparseable Cargo message")?;
            if let Message::CompilerArtifact(artifact) = message {
                if let Some(executable) = artifact.executable {
                    executables.insert(artifact.target.name, executable);
                }
            }
        }

        let status = proc.wait().wrap_err("waiting on Cargo failed")?;

        if status.success() {
            Ok(BuildOutput { executables })
        } else {
            bail!("cargo failed with {status}")
        }
    }

    /// Computes base build flags for the given platform
    fn flags_for(&self, platform: Platform) -> Flags {
        match platform {
            Platform::X86_64 => Flags {
                target_triple: "x86_64-unknown-none".to_string(),
                build_flags: vec![
                    // "-Zbuild-std=core,compiler_builtins,alloc".to_string(),
                    // "-Zbuild-std-features=compiler-builtins-mem".to_string(),
                ],
                rust_flags: vec!["-Cforce-unwind-tables".to_string()],
                cxx_flags: vec!["-fno-stack-protector".to_string()],
            },
        }
    }
}

impl BuildOutput {
    pub fn executable<'a>(&'a self, crate_name: &str) -> Result<&'a Utf8Path> {
        self.executables
            .get(crate_name)
            .map(|p| p.as_path())
            .ok_or_else(|| eyre!("no executable for crate {crate_name}"))
    }
}
