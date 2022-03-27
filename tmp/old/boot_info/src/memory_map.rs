use core::cmp::Ordering;
use core::fmt;
use core::iter::DoubleEndedIterator;

use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::PhysAddr;

/// Maximum number of memory map entries. This is a static limit so that the boot info
/// structure is a fixed size.
pub const MAX_ENTRIES: usize = 64;

#[derive(Copy, Clone)]
pub struct MemoryMap {
    // TODO: would FixedVec be a better fit here?
    entries: [Option<MemoryRegion>; MAX_ENTRIES],
}

/// Physical memory map, describing the contents and usability of memory
impl MemoryMap {
    pub fn new() -> MemoryMap {
        MemoryMap {
            entries: [None; MAX_ENTRIES],
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryRegion> + DoubleEndedIterator {
        self.entries.iter().flatten()
    }

    pub fn set_entry(&mut self, index: usize, region: MemoryRegion) {
        self.entries[index] = Some(region);
    }

    /// Called when all entries have been added to the memory map
    pub fn finish(&mut self) {
        self.entries.sort_unstable_by(|a, b| {
            // Sort None after Some, so all filled entries are at the start
            match (a, b) {
                (None, Some(_)) => Ordering::Greater,
                (Some(_), None) => Ordering::Less,
                (Some(ref a), Some(ref b)) => a.range.start.cmp(&b.range.start),
                (None, None) => Ordering::Equal,
            }
        })
    }
}

impl fmt::Debug for MemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_tuple("MemoryMap");
        self.iter().for_each(|entry| {
            builder.field(entry);
        });
        builder.finish()
    }
}

impl fmt::Display for MemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for region in self.iter() {
            writeln!(
                f,
                "* {:#x} - {:#x}: {} pages, {:?}, {:?}",
                region.start_address(),
                region.end_address(),
                region.frame_count(),
                region.kind(),
                region.usability()
            )?;
        }
        Ok(())
    }
}

/// Description of a region of physical memory
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion {
    range: PhysFrameRange,
    kind: MemoryKind,
    usability: MemoryUsability,
}

impl MemoryRegion {
    pub fn new(
        range: PhysFrameRange,
        kind: MemoryKind,
        usability: MemoryUsability,
    ) -> MemoryRegion {
        MemoryRegion {
            range,
            kind,
            usability,
        }
    }

    /// The kind of memory this region contains
    pub fn kind(&self) -> MemoryKind {
        self.kind
    }

    /// How the kernel can use this region
    pub fn usability(&self) -> MemoryUsability {
        self.usability
    }

    /// Extent of this region, in physical page frames
    pub fn range(&self) -> PhysFrameRange {
        self.range
    }

    /// Starting address of this region
    pub fn start_address(&self) -> PhysAddr {
        self.range.start.start_address()
    }

    /// Ending address of this region (exclusive)
    pub fn end_address(&self) -> PhysAddr {
        self.range.end.start_address()
    }

    /// The size (in page frames) of this region
    pub fn frame_count(&self) -> usize {
        (self.range.end - self.range.start) as usize
    }
}

/// Describes whether/how the OS can use a memory region
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemoryUsability {
    /// This region is immediately usable
    Usable,

    /// This region contains bootloader or UEFI boot services code or data that the kernel can reclaim
    BootReclaimable,

    /// This region contains ACPI tables which the kernel can reclaim when it no longer needs them
    AcpiReclaimable,

    /// This region contains kernel code/data
    Kernel,

    /// This region contains the initial kernel page table, and can be used after
    /// the kernel creates its own page table
    InitialPageTable,

    /// This region is used by UEFI runtime services
    UefiRuntime,

    /// This region is otherwise not usable. For example, it may be corrupt or for memory-mapped device I/O
    Reserved,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemoryKind {
    /// Conventional system RAM
    Conventional,

    /// Non-volatile memory
    Persistent,

    /// ACPI non-volatile storage reserved by the firmware
    AcpiNonVolatile,

    /// A region used for memory-mapped I/O by the firmware
    MemoryMappedIo,

    /// Unusable memory containing errors
    Unusable,

    /// An unkown memory type reported by the UEFI memory map
    Other {
        /// The UEFI memory type
        uefi_type: u32,
    },
}
