#![no_std]

use core::fmt;

use x86_64::PhysAddr;

pub mod memory_map;

use memory_map::MemoryMap;

#[derive(Debug, Copy, Clone)]
pub struct BootInfo {
    rsdp_address: PhysAddr,
    memory_map: MemoryMap,
}

impl BootInfo {
    pub fn new(rsdp_address: PhysAddr, memory_map: MemoryMap) -> BootInfo {
        BootInfo { rsdp_address, memory_map }
    }

    /// Physical address of the ACPI RSDP (root system description pointer)
    pub fn rsdp_address(&self) -> PhysAddr {
        self.rsdp_address
    }

    /// Map of physical memory
    pub fn memory_map(&self) -> &MemoryMap {
        &self.memory_map
    }
}

impl fmt::Display for BootInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "RSDP address = {:#x}", self.rsdp_address)?;
        writeln!(f, "Physical memory map:")?;
        fmt::Display::fmt(&self.memory_map, f)
    }
}