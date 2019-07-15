#![no_std]

pub mod addr;

pub use addr::{VirtualAddress, PhysicalAddress};

/// Main interface to a PAL implementation
pub trait Platform {
    /// The size of a single page of virtual memory, in bytes. This is the smallest unit at which
    /// memory protections and address space mappings are defined.
    const PAGE_SIZE: usize;

    /// Halt the processor such that it will not resume.
    fn halt() -> !;
}