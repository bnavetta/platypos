//! x86_64-specific code

// reexports for cross-platform compatibility
pub use x86_64::instructions::interrupts::without_interrupts;
pub use x86_64_ext::instructions::hlt_loop as halt_processor;

mod entry;
pub mod mm;
