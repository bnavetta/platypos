//! Platform services for managing memory

use core::fmt::Debug;
use core::ops::Deref;

use crate::Platform;
use crate::addr::{VirtualAddress, PhysicalAddress};

/// A virtual address space. The exact details vary depending on hardware.
///
/// Multiple processors must be able to use the same address space concurrently (for example, if
/// a process has several threads executing at once, all scheduled on different processors). Any
/// changes to the address space must be synchronized between those processors.
pub trait AddressSpace<P: Platform>: Send + Sync {
    /// A thread-safe handle to an address space which the kernel can safely pass around. This will
    /// usually be an Arc, but in some cases may be a reference or the AddressSpace type itself.
    type Reference: Send + Sync + Deref<Target=Self>;

    /// An error which can occur when mapping a page into this address space.
    type MapError: Debug;

    /// An error which can occur when removing a mapping from this address space.
    type UnmapError: Debug;

    /// Access the active address space on the current processor.
    fn current() -> Self::Reference;

    /// Switch to a different address space.
    ///
    /// # Unsafety
    /// Changing the active address space can violate memory safety.
    unsafe fn switch(to: Self::Reference);

    /// Translate a virtual address in this address space to the physical address it refers to.
    /// If the given virtual address is not mapped to a physical address, returns `None`.
    fn translate(&self, vaddr: VirtualAddress) -> Option<PhysicalAddress>;

    /// Add a new mapping to this address space
    ///
    /// # Arguments
    /// * `page_start` - the first address of the page (must be page-aligned)
    /// * `frame_start` - the first address of the frame (must be page-aligned)
    ///
    /// # Unsafety
    /// Changing the page tables can cause memory safety violations.
    // TODO: support flags
    unsafe fn map_page(&self, page_start: VirtualAddress, frame_start: PhysicalAddress) -> Result<(), Self::MapError>;

    /// Remove a mapping from this address space
    ///
    /// # Arguments
    /// * `page_start` - the first address of the page to unmap (must be page-aligned)
    ///
    /// # Unsafety
    /// Changing the page tables can cause memory safety violations.
    unsafe fn unmap_page(&self, page_start: VirtualAddress) -> Result<PhysicalAddress, Self::UnmapError>;
}