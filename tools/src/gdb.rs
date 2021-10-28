use std::{path::Path, process::Command};
use std::io::Write;

use miette::{Context, IntoDiagnostic, Result, miette};
use owo_colors::OwoColorize;
use tempfile::{NamedTempFile, TempPath};

use crate::build::BuildInfo;

pub fn debugger(root: &Path, build_info: &BuildInfo) -> Result<()> {
    let gdbinit = generate_gdbinit(build_info)?;

    let mut gdb_cmd = Command::new("ugdb");
    gdb_cmd
        .args(&["--nh", "--gdb", "riscv64-linux-gnu-gdb"])
        .arg("--cd").arg(root)
        .arg("-x").arg(&gdbinit);
    println!("Running {:?}", gdb_cmd.green());

    let status = gdb_cmd.status()
        .into_diagnostic()
        .wrap_err("Running GDB failed")?;

    if status.success() {
        Ok(())
    } else {
        Err(miette!("GDB exited with {}", status))
    }
}

fn generate_gdbinit(build_info: &BuildInfo) -> Result<TempPath> {
    let mut file = NamedTempFile::new()
        .into_diagnostic()
        .wrap_err("Could not create temporary .gdbinit file")?;

    writeln!(&mut file, "target remote :1234").into_diagnostic()?;
    writeln!(&mut file, "file {}", build_info.kernel_elf.display()).into_diagnostic()?;
    writeln!(&mut file, "break kmain").into_diagnostic()?;

    Ok(file.into_temp_path())
}