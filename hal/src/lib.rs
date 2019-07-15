//! PlatypOS hardware abstraction layer
#![no_std]
pub use crate::addr::{PhysicalAddress, VirtualAddress};
use crate::services::memory::AddressSpace;

pub mod services;
mod addr;

/// A hardware platform. Platforms roughly encompass CPU architecture and standard associated hardware.
/// For example, the x86-64 platform includes both an address space implementation and APIC support.
pub trait Platform: 'static + Sized {
    // Associated constants for platform details

    /// The size of a single page of virtual memory, in bytes. This is the smallest unit at which
    /// memory protections and address space mappings are defined.
    const PAGE_SIZE: usize;

    // Associated types for services
    type AddressSpace: AddressSpace<Self>;

    /// Get the ID of the processor executing the calling code.
    fn current_processor(&self) -> ProcessorId;
}

/// Logical processor identifier. HAL implementations are responsible for mapping between these
/// identifiers and hardware-specific ones.
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct ProcessorId(usize);

impl core::fmt::Display for ProcessorId {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "Processor {}", self.0)
    }
}
