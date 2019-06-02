use x86_64::instructions::hlt;

pub mod core_local;

/// Infinite loop executing the hlt instruction.
pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}
