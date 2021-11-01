//! Building PlatypOS

use std::fs;
use std::io::BufReader;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;
use std::process::ExitStatus;
use std::process::Stdio;

use cargo_metadata::Artifact;
use cargo_metadata::Message;
use miette::Context;
use miette::Diagnostic;
use miette::IntoDiagnostic;
use miette::Result;
use owo_colors::OwoColorize;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Release,
    Debug,
}

pub struct BuildInfo {
    pub kernel_elf: PathBuf,
    pub kernel_binary: PathBuf,
}

#[derive(Error, Diagnostic, Debug)]
#[error("cargo build for {crate_name} failed with {status}")]
#[diagnostic(
    code(platypos::build::cargo),
    help("check the cargo output for details")
)]
pub struct CargoError {
    crate_name: &'static str,
    status: ExitStatus,
}

#[derive(Error, Diagnostic, Debug)]
#[error("objcopy {src} -> {dest} failed with {status}")]
#[diagnostic(
    code(platypos::build::objcopy),
    help("check the objcopy output for details")
)]
pub struct ObjcopyError {
    src: PathBuf,
    dest: PathBuf,
    status: ExitStatus,
}

const KERNEL_CRATE: &'static str = "kernel";

pub fn build_kernel(root: &Path, mode: Mode) -> Result<BuildInfo> {
    println!("Compiling {}...", KERNEL_CRATE.green());
    let kernel_dir = root.join(KERNEL_CRATE);

    let mut build_cmd = Command::new("cargo");
    build_cmd
        .current_dir(kernel_dir)
        .args(&["build", "--message-format=json-render-diagnostics"])
        .stdout(Stdio::piped());

    if mode == Mode::Release {
        build_cmd.arg("--release");
    }

    let mut build_cmd = build_cmd
        .spawn()
        .into_diagnostic()
        .wrap_err("Launching Cargo failed")?;

    let cargo_out = BufReader::new(build_cmd.stdout.take().expect("missing stdout"));
    let mut kernel_elf = None;
    // If true, the already-existing file was up to date
    let mut kernel_fresh = false;
    // Consume all messages to not fill stdout buffer
    for message in Message::parse_stream(cargo_out) {
        let message = message
            .into_diagnostic()
            .wrap_err("could not read cargo messages")?;
        if let Message::CompilerArtifact(artifact) = message {
            if is_kernel(&artifact) {
                kernel_elf = artifact.executable;
                kernel_fresh = artifact.fresh;
            }
        }
    }

    let build_res = build_cmd
        .wait()
        .into_diagnostic()
        .wrap_err("Waiting for Cargo failed")?;

    if !build_res.success() {
        return Err(CargoError {
            crate_name: KERNEL_CRATE,
            status: build_res,
        }
        .into());
    }

    let kernel_elf = kernel_elf.expect("could not find kernel executable in Cargo output");
    let mut kernel_binary = kernel_elf.clone();
    kernel_binary.set_extension("bin");

    if !kernel_fresh || more_recent(&kernel_elf, &kernel_binary) {
        println!("Creating flat binary...");

        let mut copy_cmd = Command::new("rust-objcopy");
        copy_cmd
            .arg(&kernel_elf)
            .args(&["--binary-architecture=riscv64", "-O", "binary"]);
        if mode == Mode::Release {
            copy_cmd.arg("--strip-all");
        }
        copy_cmd.arg(&kernel_binary);

        let copy_res = copy_cmd
            .spawn()
            .into_diagnostic()
            .wrap_err("Launching objcopy failed")?
            .wait()
            .into_diagnostic()
            .wrap_err("Waiting for objcopy failed")?;

        if !copy_res.success() {
            return Err(ObjcopyError {
                src: kernel_elf.into_std_path_buf(),
                dest: kernel_binary.into_std_path_buf(),
                status: copy_res,
            }
            .into());
        }
    } else {
        println!("Kernel binary up to date.")
    }

    println!("Built kernel at {}", kernel_binary.blue());

    Ok(BuildInfo {
        kernel_elf: kernel_elf.into_std_path_buf(),
        kernel_binary: kernel_binary.into_std_path_buf(),
    })
}

/// Checks if a compiler artifact is the PlatypOS kernel
fn is_kernel(artifact: &Artifact) -> bool {
    artifact.package_id.repr.starts_with("platypos_kernel")
}

/// Returns `true` if `a` was modified more recently than `b`.
/// Missing files are considered older than existing ones.
fn more_recent<P1: AsRef<Path>, P2: AsRef<Path>>(a: P1, b: P2) -> bool {
    let a_mtime = match fs::metadata(a).and_then(|m| m.modified()) {
        Ok(mtime) => mtime,
        Err(_) => return false,
    };

    let b_mtime = match fs::metadata(b).and_then(|m| m.modified()) {
        Ok(mtime) => mtime,
        Err(_) => return true,
    };

    a_mtime > b_mtime
}

#[cfg(test)]
mod test {
    use super::more_recent;

    use tempfile::NamedTempFile;

    #[test]
    fn more_recent_comparision() {
        assert!(!more_recent("missing", "also_missing"));

        let exists = NamedTempFile::new().unwrap().into_temp_path();
        assert!(more_recent(&exists, "missing"));
        assert!(!more_recent("missing", &exists));

        let exists_newer = NamedTempFile::new().unwrap().into_temp_path();
        assert!(more_recent(&exists_newer, &exists));
        assert!(!more_recent(&exists, &exists_newer));
    }
}
