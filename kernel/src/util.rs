use x86_64::instructions::hlt;

pub mod core_local;
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
