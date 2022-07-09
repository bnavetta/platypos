#![no_std]

use linkme::distributed_slice;
use qemu_exit::QEMUExit;

pub mod assertions;

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

#[doc(hidden)]
#[distributed_slice]
pub static TESTS: [Test] = [..];

// ::core::arch::global_asm!(r#".section linkme_TESTS,"aR",@progbits"#);

#[distributed_slice(TESTS)]
static CORE: Test = Test::new("core_test", core_test);

fn core_test() -> Outcome {
    Outcome::Pass
}

/// Test framework entry point. The kernel calls this when running in test mode,
/// after performing the bare minimum platform setup (for example, initializing
/// logging and memory allocation).
pub fn run_tests() -> ! {
    let _enter = tracing::info_span!("ktest::run_tests");

    let test_addr = &TESTS as *const _ as usize;

    tracing::info!(at = test_addr, "Running tests");

    tracing::info!("Running {} kernel tests", TESTS.len());
    let mut failures = 0;

    for test in TESTS {
        let result = (test.imp)();
        match result {
            Outcome::Pass => tracing::info!("{}... OK", test.name),
            Outcome::Fail => {
                failures += 1;
                tracing::error!("{}... FAIL", test.name);
            }
        }
    }
    tracing::info!(
        "Done! {} passed and {} failed",
        TESTS.len() - failures,
        failures
    );

    exit(failures == 0);
}

impl Test {
    pub const fn new(name: &'static str, imp: fn() -> Outcome) -> Self {
        Test { name, imp }
    }
}

/// Exits the VM
pub fn exit(success: bool) -> ! {
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
