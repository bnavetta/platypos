//! Structures for handing off between bootloaders and the PlatypOS kernel on x86-64.
#![no_std]

use x86_64::PhysAddr;
use x86_64::structures::paging::frame::PhysFrame;

use platypos_pal::mem::map::{self, MemoryRegionFlags};

/// x86-64 boot info structure
#[repr(C)] // should be same layout for UEFI and kernel targets
#[derive(Debug)]
pub struct BootInfo {
    /// A list of memory region descriptors
    memory_map: [Option<MemoryRegion>; map::MAX_ENTRIES],

    rsdp: PhysAddr,
}

/// A region of physical memory. This structure is intentionally barebones; the kernel should convert
/// it to a higher level PAL `MemoryRegion` as soon as possible.
#[derive(Debug, Copy, Clone)]
pub struct MemoryRegion {
    pub start: PhysFrame,
    pub frames: usize,
    pub flags: MemoryRegionFlags
}

impl BootInfo {
    pub fn memory_map(&self) -> impl Iterator<Item=&MemoryRegion> {
        self.memory_map.iter().filter_map(|r| match r {
            // Convert &Option(MemoryRegion) to &MemoryRegion, while filtering out unused entries
            Some(ref entry) => Some(entry),
            None => None
        })
    }

    /// Writes the memory map into this `BootInfo` structure.
    ///
    /// # Panics
    /// If `from` contains more entries than can be stored in the `BootInfo` structure. The limit
    /// is defined in `platypos_pal::mem::map::MAX_ENTRIES`.
    pub fn set_memory_map<I: IntoIterator<Item=MemoryRegion>>(&mut self, from: I) {
        let mut i = 0;
        for region in from.into_iter() {
            if i >= map::MAX_ENTRIES {
                panic!("memory map contains more than MAX_ENTRIES entries");
            }
            self.memory_map[i] = Some(region);
            i += 1
        }

        for extra in i..map::MAX_ENTRIES {
            self.memory_map[extra] = None;
        }
    }

    /// The RSDP (root system description pointer), which is used to find ACPI tables and get
    /// other ACPI information.
    pub fn rsdp(&self) -> PhysAddr {
        self.rsdp
    }

    /// Writes the RSDP into this `BootInfo` structure.
    pub fn set_rsdp(&mut self, rsdp: PhysAddr) {
        self.rsdp = rsdp;
    }
}