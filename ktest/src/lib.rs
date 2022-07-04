#![no_std]

use core::fmt::Write;

use ansi_rgb::{green, red, Foreground};
use assertions::ASSERTION_OUTPUT;
use linkme::distributed_slice;
use qemu_exit::QEMUExit;

mod assertions;

pub use assertions::*;

pub struct Test {
    name: &'static str,
    imp: fn() -> Outcome,
    // TODO: support should_fail, etc.
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Pass,
    Fail,
}

#[distributed_slice]
#[doc(hidden)]
pub static TESTS: [Test] = [..];

/// Test framework entry point. The kernel calls this when running in test mode,
/// after performing the bare minimum platform setup (for example, initializing
/// logging and memory allocation).
pub fn run_tests<W: Write + Send + Sync + 'static>(out: &'static mut W) -> ! {
    writeln!(out, "Running {} kernel tests", TESTS.len()).unwrap();
    let mut failures = 0;

    {
        *ASSERTION_OUTPUT.lock() = Some(out);
    }

    for test in TESTS {
        {
            write!(
                ASSERTION_OUTPUT.lock().as_mut().unwrap(),
                "{}...",
                test.name
            )
            .unwrap();
        }

        let result = (test.imp)();
        match result {
            Outcome::Pass => writeln!(
                ASSERTION_OUTPUT.lock().as_mut().unwrap(),
                " {}",
                "OK".fg(green())
            )
            .unwrap(),
            Outcome::Fail => {
                failures += 1;
                writeln!(
                    ASSERTION_OUTPUT.lock().as_mut().unwrap(),
                    " {}",
                    "FAIL".fg(red())
                )
                .unwrap();
            }
        }
    }
    writeln!(
        ASSERTION_OUTPUT.lock().as_mut().unwrap(),
        "Done! {} passed and {} failed",
        TESTS.len() - failures,
        failures
    )
    .unwrap();

    exit(failures == 0);
}

fn exit(success: bool) -> ! {
    #[cfg(target_arch = "x86_64")]
    let handle = qemu_exit::X86::new(0xf4, 3);
    #[cfg(not(target_arch = "x86_64"))]
    compile_error!("QEMU exit not configured for {}" cfg!(target_arch));

    if success {
        handle.exit_success()
    } else {
        handle.exit_failure()
    }
}
