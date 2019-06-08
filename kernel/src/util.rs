use core::time::Duration;

use x86_64::instructions::hlt;

use crate::time::current_timestamp;
use core::hint::spin_loop;

pub mod processor_local;
pub mod qemu;

/// Infinite loop executing the hlt instruction.
pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}

/// Calculates the smallest number of pages needed to contain `size` bytes
pub fn page_count(size: usize) -> usize {
    (size + 4096) / 4096
}

/// Spin until the given condition is true or the timeout has elapsed
pub fn spin_on<F>(mut condition: F, timeout: Duration) -> bool
where
    F: FnMut() -> bool,
{
    let deadline = current_timestamp() + timeout;

    while current_timestamp() < deadline {
        if condition() {
            return true;
        }
        spin_loop();
    }

    false
}
