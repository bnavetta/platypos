//! GDB setup

use std::fs::{self, File};
use std::io::{ErrorKind, Write};

use lazy_static::lazy_static;

use crate::prelude::*;

lazy_static! {
    static ref GDB_DIR: &'static Utf8Path = Utf8Path::new("target/gdb");
    static ref SOCKET_PATH: Utf8PathBuf = GDB_DIR.join("gdb.sock");
    static ref INIT_PATH: Utf8PathBuf = GDB_DIR.join("gdbinit");
}

/// Handle for the server side of a GDB session. GDB state is automatically
/// cleaned up when it's dropped, so it should outlive the QEMU invocation.
pub struct Server {
    wait: bool,
}

impl Server {
    pub fn new(target_binary: &Utf8Path, wait: bool) -> Result<Server> {
        fs::create_dir_all(&*GDB_DIR)?;

        if SOCKET_PATH.as_std_path().exists() {
            bail!(
                "{} already exists - is another QEMU instance already running?",
                &*SOCKET_PATH
            );
        }

        let mut w = File::create(&*INIT_PATH)?; // Will truncate if needed
        write_config(target_binary, &mut w)
            .wrap_err_with(|| format!("could not write GDB config file {}", &*INIT_PATH))?;

        Ok(Self { wait })
    }

    /// Path to the GDB Unix socket
    pub fn socket_path(&self) -> &Utf8Path {
        &SOCKET_PATH
    }

    /// Should QEMU wait for GDB to attach?
    pub fn should_wait(&self) -> bool {
        self.wait
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        // Clean these up so that they're not picked up by subsequent runs
        try_remove(&SOCKET_PATH);
        try_remove(&INIT_PATH);
    }
}

/// Runs GDB
pub fn run() -> Result<()> {
    if !&INIT_PATH.exists() {
        bail!("Expected GDB config at {}. Is QEMU running?", &*INIT_PATH);
    }

    if !&SOCKET_PATH.exists() {
        bail!("Expected GDB socket at {}. Is QEMU running?", &*SOCKET_PATH);
    }

    duct::cmd!("rust-gdb", "-x", &*INIT_PATH).run()?;

    Ok(())
}

fn try_remove(path: &Utf8Path) {
    if let Err(err) = fs::remove_file(path) {
        if !matches!(err.kind(), ErrorKind::NotFound) {
            eprintln!(
                "Could not remove {}: {}",
                path.if_supports_color(Stream::Stderr, |p| p.red()),
                err
            );
        }
    }
}

/// Writes the GDB configuration file
fn write_config<W: Write>(target_binary: &Utf8Path, file: &mut W) -> Result<()> {
    writeln!(file, "target remote {}", &*SOCKET_PATH)?;
    writeln!(file, "add-symbol-file {}", target_binary)?;
    writeln!(file, "tui enable")?;
    writeln!(file, "hbreak platypos_kernel::panic::panic")?;
    Ok(())
}
