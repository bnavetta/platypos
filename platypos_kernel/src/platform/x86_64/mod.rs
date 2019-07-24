use x86_64::instructions::hlt;

// Modules in the common platform API
pub mod addr;
pub mod memory;
pub mod processor;

// Modules internal to x86-64
mod apic;

mod entry;

pub use addr::{PhysicalAddress, VirtualAddress};

/// Halt the processor. This function is guaranteed to never return, although interrupt handlers may
/// still be called.
pub fn halt() -> ! {
    loop {
        hlt()
    }
}

pub fn init_perprocessor_data() {
    use log::info;

    info!("per-processor data start: {:p}", unsafe {
        &PERPROCESSOR_START
    });
    info!("per-processor data end: {:p}", unsafe { &PERPROCESSOR_END });

    info!("FOO is {}", unsafe { FOO });
}

extern "C" {
    static PERPROCESSOR_START: usize;
    static PERPROCESSOR_END: usize;
}

#[link_section = ".perprocessor"]
static mut FOO: usize = 1;
