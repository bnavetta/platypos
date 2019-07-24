use core::iter::FromIterator;

use x86_64::PhysAddr;

pub const MAX_ENTRIES: usize = 64; // seems reasonable?

/// Map of physical memory. To save space, this map only contains regions
/// that are usable by the OS or required for UEFI runtime services
#[derive(Copy, Clone)]
pub struct MemoryMap {
    entries: [Option<MemoryRegion>; MAX_ENTRIES],
}

impl MemoryMap {
    pub fn new() -> MemoryMap {
        MemoryMap {
            entries: [None; MAX_ENTRIES]
        }
    }

    pub fn set_entry(&mut self, index: usize, entry: MemoryRegion) {
        assert!(index < MAX_ENTRIES, "MemoryMap can only hold {} entries", MAX_ENTRIES);
        self.entries[index] = Some(entry);
    }

    pub fn iter(&self) -> impl Iterator<Item = &MemoryRegion> {
        self.entries.iter().flatten()
    }
}

impl FromIterator<MemoryRegion> for MemoryMap {
    fn from_iter<T: IntoIterator<Item=MemoryRegion>>(iter: T) -> Self {
        let mut map = MemoryMap::new();
        for (i, region) in iter.into_iter().enumerate() {
            assert!(i <= MAX_ENTRIES, "MemoryMap can only hold {} entries", MAX_ENTRIES);
            map.entries[i] = Some(region);
        }

        map
    }
}

#[derive(Debug, Copy, Clone)]
pub struct MemoryRegion {
    kind: MemoryKind,
    uefi_runtime: bool,
    start: PhysAddr,
    frames: usize,
}

impl MemoryRegion {
    pub fn new(kind: MemoryKind, uefi_runtime: bool, start: PhysAddr, frames: usize) -> MemoryRegion {
        MemoryRegion {
            kind, uefi_runtime, start, frames
        }
    }

    /// The kind of memory this is
    pub fn kind(&self) -> MemoryKind {
        self.kind
    }

    /// Whether this memory is needed by UEFI runtime services
    pub fn uefi_runtime(&self) -> bool {
        self.uefi_runtime
    }

    /// Starting address of the region
    pub fn start(&self) -> PhysAddr {
        self.start
    }

    /// Size of the region in 4KiB page frames
    pub fn frames(&self) -> usize {
        self.frames
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum MemoryKind {
    /// Memory containing the kernel code and data
    Kernel,

    /// The initial kernel page table allocated by the bootloader. This is
    /// reclaimable once the kernel has created new page tables
    KernelPageTable,

    /// Memory containing bootloader code and data
    Bootloader,

    /// Memory containing the UEFI boot services. This is reclaimable
    BootServices,

    /// Memory containing UEFI runtime services. This is not reclaimable
    RuntimeServices,

    /// Regular usable memory
    Conventional,

    /// Memory containing ACPI tables which can be reclaimed once those tables
    /// have been parsed
    AcpiReclaimable,

    /// Some other memory type
    Other {
        /// The UEFI memory type
        uefi_type: u32,
    }
}