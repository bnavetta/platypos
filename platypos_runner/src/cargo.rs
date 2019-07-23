use std::env;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use ansi_term::{ANSIStrings, Style, Color};
use cargo_metadata::{Message, Metadata};
use failure::{Error, format_err};

/// Builds a package with cargo-xbuild for the given target, returning the path to the built
/// executable
pub fn build_package(metadata: &Metadata, package: &str, target: &str) -> Result<PathBuf, Error> {
    println!("{}", ANSIStrings(&[
        Style::new().paint("Building "),
        Color::Green.paint(package),
        Style::new().paint(" for "),
        Color::Green.paint(target),
    ]));

    let cargo = env::var("CARGO").unwrap_or("cargo".to_string());

    let mut child = Command::new(cargo)
        .arg("xbuild")
        .arg("-p")
        .arg(package)
        .arg("--target")
        .arg(target)
        .arg("--target-dir")
        .arg(&metadata.target_directory)
        .arg("--message-format=json")
        .stdout(Stdio::piped())
        // The bootimage crate does this to set the cargo-xbuild sysroot path, which avoids lock contention on the kernel sysroot
        .env_remove("RUSTFLAGS")
        .env("XBUILD_SYSROOT_PATH", metadata.target_directory.join("loader-sysroot"))
        .spawn()?;

    let mut executable: Option<PathBuf> = None;
    for message in cargo_metadata::parse_messages(child.stdout.take().expect("Could not get child stdout")) {
        if let Message::CompilerArtifact(artifact) = message? {
            if artifact.target.name == package && artifact.target.kind.contains(&"bin".to_string()) {
                executable = artifact.executable;
            }
        }
    }

    let status = child.wait().expect("Couldn't get cargo exit status");
    if status.success() {
        executable.ok_or(format_err!("Executable for {} not found", package))
    } else {
        Err(format_err!("Cargo failed with status {}", status))
    }
}