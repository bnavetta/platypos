#![no_std]
#![feature(custom_test_frameworks)]
#![test_runner(platypos_test::test_runner)]

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

#[cfg(test)]
pub mod tests {
    use platypos_test::kernel_test;

    #[kernel_test]
    fn test_foo() {

    }
}
