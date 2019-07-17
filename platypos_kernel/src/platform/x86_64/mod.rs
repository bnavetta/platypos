use x86_64::instructions::hlt;

pub mod addr;
pub mod memory;

#[cfg(not(test))]
mod entry;

pub use addr::{PhysicalAddress, VirtualAddress};

/// Halt the processor. This function is guaranteed to never return, although interrupt handlers may
/// still be called.
pub fn halt() -> ! {
    loop {
        hlt()
    }
}
