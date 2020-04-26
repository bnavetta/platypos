use x86_64::instructions::hlt;

/// Infinitely loop calling `hlt`. Useful for fatal conditions in code that cannot panic or otherwise report them.
pub fn hlt_loop() -> ! {
    loop {
        hlt();
    }
}