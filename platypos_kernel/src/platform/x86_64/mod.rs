use x86_64::instructions::hlt;

pub mod addr;

#[cfg(not(test))]
mod entry;

pub use addr::{PhysicalAddress, VirtualAddress};

pub fn halt() -> ! {
    loop {
        hlt()
    }
}
