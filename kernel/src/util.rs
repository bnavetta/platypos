use x86_64::instructions::hlt;

/// Infinite loop executing the hlt instruction.
pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}