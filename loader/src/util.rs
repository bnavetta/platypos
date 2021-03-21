use core::mem;
use core::slice;

use uefi::prelude::*;
use uefi::table::boot::{AllocateType, MemoryDescriptor, MemoryType};
use x86_64::PhysAddr;

use crate::PAGE_SIZE;

/// Allocate `count` frames of physical memory, returning both the starting physical address and the memory region as a slice.
/// The allocated memory is **not** zeroed.
pub fn allocate_frames(
    system_table: &SystemTable<Boot>,
    count: usize,
    typ: MemoryType,
) -> (PhysAddr, &'static mut [u8]) {
    let phys_addr = match system_table
        .boot_services()
        .allocate_pages(AllocateType::AnyPages, typ, count)
        .log_warning()
    {
        Ok(addr) => addr,
        Err(err) => panic!("Could not allocate {} frames: {:?}", count, err),
    };

    // Safety: this address was just returned from the BootServices page allocator, so it's guaranteed to be usable and free
    let buf =
        unsafe { slice::from_raw_parts_mut(phys_addr as *mut u8, count * PAGE_SIZE as usize) };
    (PhysAddr::new(phys_addr), buf)
}

/// Calculates the buffer size needed to hold a UEFI memory map. This includes extra padding beyond what the system indicates, because the given size is usually a bit too small.
pub fn memory_map_size(system_table: &SystemTable<Boot>) -> usize {
    system_table.boot_services().memory_map_size() + 2 * mem::size_of::<MemoryDescriptor>()
}
