//! Physical memory map definitions. The exact details of the physical address space, like whether
//! certain regions are reserved for firmware or if some memory is non-volatile, are highly platform-
//! and system-dependent. Having a common representation means we don't need quite as much
//! platform-specific memory management code

use core::fmt;
use core::iter::FromIterator;

use arrayvec::ArrayVec;

use crate::Platform;
use super::PageFrameRange;

/// Maximum number of memory map entries supported.
pub const MAX_ENTRIES: usize = 64;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct MemoryMap<P: Platform> {
    entries: ArrayVec<[MemoryRegion<P>; MAX_ENTRIES]>
}

/// A region of physical memory
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion<P: Platform> {
    range: PageFrameRange<P>,
    flags: MemoryRegionFlags,
}

bitflags! {
    pub struct MemoryRegionFlags: u32 {
        /// This region contains usable memory.
        const USABLE = 0b0000000000000001;

        /// This region contains non-volatile memory.
        const NON_VOLATILE = 0b0000000000000010;

        /// This region is part of the memory-mapped I/O space (ex. for PCIe devices)
        const MEMORY_MAPPED_IO = 0b0000000000001000;

        /// This region is used for memory-mapped port I/O
        const MEMORY_MAPPED_PORT_IO = 0b0000000000010000;

        /// This region is reserved by firmware or a device and cannot be used.
        const RESERVED = 0b0000000000100000;

        /// This region contains part of the UEFI runtime services.
        const UEFI_RUNTIME = 0b0000000001000000;

        /// This region contains ACPI tables. It can be used if those tables are no longer needed.
        const ACPI_TABLES = 0b0000000010000000;

        /// This region is non-volatile storage for ACPI.
        const ACPI_STORAGE = 0b0000000100000000;

        /// This region contains part of the kernel executable.
        const KERNEL = 0b0000001000000000;
    }
}

/*
 UEFI types -> flags

 LOADER_CODE -> USABLE
 LOADER_DATA -> USABLE
 BOOT_SERVICES_CODE -> USABLE
 BOOT_SERVICES_DATA -> USABLE
 RUNTIME_SERVICES_CODE -> RESERVED | UEFI_RUNTIME
 RUNTIME_SERVICES_DATA -> RESERVED | UEFI_RUNTIME
 CONVENTIONAL_MEMORY -> USABLE
 UNUSABLE_MEMORY -> ???
 ACPI_RECLAIM -> ACPI_TABLES | USABLE?
 ACPI_NVS -> NON_VOLATILE | RESERVED | ACPI_STORAGE
 MMIO -> MEMORY_MAPPED_IO
 MMIO_PORTS -> MEMORY_MAPPED_IO_PORTS
 PAL_CODE -> RESERVED
 PERSISTENT -> USABLE | NON_VOLATILE
 */

impl <P: Platform> MemoryMap<P> {
    /// An iterator over the memory map's entries. Entries are ordered by their starting address.
    pub fn iter(&self) -> impl Iterator<Item=&MemoryRegion<P>> {
        self.entries.iter()
    }
}

impl <P: Platform> FromIterator<MemoryRegion<P>> for MemoryMap<P> {
    /// Create a new memory map from an iterator of entries.
    ///
    /// # Panics
    /// If `entries` is longer than `MAX_ENTRIES`. If we can't pass the kernel a memory map, it can't
    /// start up, so this isn't a recoverable error. This may require increasing `MAX_ENTRIES`.
    fn from_iter<I: IntoIterator<Item=MemoryRegion<P>>>(iter: I) -> MemoryMap<P> {
        let mut entries = ArrayVec::new();
        for entry in iter {
            entries.try_push(entry).expect("memory map contained more than MAX_ENTRIES entries");
        }
        MemoryMap { entries }
    }
}

impl <P: Platform> MemoryRegion<P> {
    /// Create a new memory region.
    pub fn new(range: PageFrameRange<P>, flags: MemoryRegionFlags) -> MemoryRegion<P> {
        MemoryRegion { range, flags }
    }
}

impl <P: Platform> fmt::Display for MemoryRegion<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {:?}", self.range, self.flags)
    }
}