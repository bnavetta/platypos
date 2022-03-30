use x86_64::instructions::interrupts;

/// Guard for disabling interrupts around a critical section of code.
pub struct Guard {
    enable_flag: bool,
}

impl !Send for Guard {}

/// Disable interrupts on the current processor, returning a guard value.
/// When the guard is dropped, interrupts are reenabled.
#[inline(always)]
pub fn disable() -> Guard {
    let enable_flag = interrupts::are_enabled();

    // If interrupts were enabled before, disable them while the guard is active
    if enable_flag {
        interrupts::disable();
    }

    Guard { enable_flag }
}

impl Drop for Guard {
    #[inline(always)]
    fn drop(&mut self) {
        if self.enable_flag {
            interrupts::enable();
        }
    }
}

/// Halts the processor until there's an interrupt
pub fn halt_until_interrupted() {
    interrupts::enable_and_hlt()
}
