#![no_std]

pub mod memory_map;

#[derive(Copy, Clone)]
pub struct BootInfo {
    memory_map: memory_map::MemoryMap,
}

impl BootInfo {
    pub fn new(memory_map: memory_map::MemoryMap) -> BootInfo {
        BootInfo {
            memory_map,
        }
    }

    pub fn memory_map(&self) -> &memory_map::MemoryMap {
        &self.memory_map
    }
}