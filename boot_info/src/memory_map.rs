use core::cmp::Ordering;
use core::fmt;

use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrameRange;

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
        MemoryMap { entries: [None; MAX_ENTRIES] }
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryRegion> {
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
                (None, None) => Ordering::Equal
            }
        })
    }
}

impl fmt::Debug for MemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut builder = f.debug_tuple("MemoryMap");
        self.iter().for_each(|entry| { builder.field(entry); });
        builder.finish()
    }
}

impl fmt::Display for MemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for region in self.iter() {
            writeln!(f, "* {:#x} - {:#x}: {} pages, {:?}", region.start_address(), region.end_address(), region.frame_count(), region.kind())?;
        }
        Ok(())
    }
}

/// Description of a region of physical memory
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct MemoryRegion {
    range: PhysFrameRange,
    kind: MemoryKind
}

impl MemoryRegion {
    pub fn new(range: PhysFrameRange, kind: MemoryKind) -> MemoryRegion {
        MemoryRegion {
            range,
            kind
        }
    }

    /// The kind of memory this region contains
    pub fn kind(&self) -> MemoryKind {
        self.kind
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

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemoryKind {
    /// Kernel code and data
    Kernel,

    /// Memory needed in the bootloader that the kernel can reclaim once it is fully initialized, such as data for UEFI
    /// boot services or the transitional kernel page table.
    BootReclaimable,

    /// Memory containing ACPI tables which can be reclaimed once the kernel has processed them.
    AcpiReclaimable,

    /// Memory needed by UEFI runtime services, which is never reclaimable.
    UefiRuntime,

    /// Regular usable memory
    Conventional,

    /// An unkown memory type reported by the UEFI memory map
    Other {
        /// The UEFI memory type
        uefi_type: u32,
    }
}