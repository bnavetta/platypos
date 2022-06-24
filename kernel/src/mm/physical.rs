//! Physical memory management (page frames)

use itertools::Itertools;

use crate::arch::mm::PhysicalMemoryAccess;
use crate::arch::{PAGE_SIZE, PhysicalPageNumber};
use crate::prelude::*;

use super::map::Region;

/// Builder for the physical memory allocator
pub struct Builder;

/// The address space may contain many small reserved/unusable memory regions.
/// To avoid fragmenting the allocator state into chunks with high bookkeeping
/// overhead, small unusable regions are merged into larger usable regions and
/// marked as unavailable.
const MAP_HOLE_THRESHOLD: usize = 4 * 1024 * 1024;

impl Builder {
    pub fn parse_memory_map<I>(
        &mut self,
        access: &mut PhysicalMemoryAccess,
        mut memory_map: I,
    ) -> Result<(), Error>
    where
        I: Iterator<Item = Region> + Clone,
    {
        let max_usable = memory_map
            .clone()
            .filter_map(|r| if r.usable() { Some(r.end()) } else { None })
            .max()
            .ok_or(Error::new(ErrorKind::InsufficientMemory))?;
        log::info!("Maximum usable address: {max_usable}");

        // This implicitly assumes that there isn't a large memory hole in low memory.
        // If there were, we might not want to waste space marking it as unusable.

        // Need one bit per page
        let page_count = max_usable.as_usize().div_ceil(PAGE_SIZE);
        let bitmap_size = page_count / u8::BITS as usize;
        log::info!("Need {} for bitmap", bitmap_size.as_size());

        let bitmap_region = memory_map
            .filter(|r| r.usable())
            .coalesce(|a, b| {
                // Combine adjacent usable regions. It's possible that the firmware provided a
                // very fragmented memory map, so coalescing makes sure that the first usable
                // bitmap space is found.
                if b.start() <= a.end() {
                    Ok(Region::new(a.kind(), a.start(), b.end()))
                } else {
                    Err((a, b))
                }
            })
            .find(|r| r.size() >= bitmap_size)
            .ok_or(Error::new(ErrorKind::InsufficientMemory))?;

        log::info!("Using {} for bitmap", bitmap_region);

        let bitmap = unsafe {
            // Safety: all usable memory is unallocated at this point, so there is not an existing mapping to alias
            // TODO: better PPN type
            let addr = access.map_permanent(todo!("PPN API"), bitmap_size.div_ceil(PAGE_SIZE))?;
            // TODO: make sure we have a whole-usize allocation - may need to round up bitmap_size for that
        }

        Ok(())

        // TODOs:
        // - macro to implement all the arithmetic+formatting for PhysicalAddress, VirtualAddress, PPN, Page
        // - API for ranges of addresses/pages/PPNs - could make them all implement a trait (instead of macro), and make the range type generic
        // - API for temporarily accessing physical memory (could return a &'a [MaybeUninit<u8>] to ensure reference doesn't escape)
        // - alternative construction algorithm to handle unusable memory like kernel:
        //     1. Find a usable page frame (in list from bootloader, doesn't overlap w/ reserved regions)
        //     2. Use that to create a scratch allocator
        //     3. Put memory into a Vec in scratch allocator - then can sort, coalesce, split out reserved regions
        // - clean up xtasks:
        //     - support building multiple platforms at once
        //     - more composable abstractions:
        //         - build crate X for platform Y, producing the binary artifact
        //         - run kernel-like artifact (platform-appropriate) on platform Y (platform knows which qemu to use, can pass generic settings like memory+CPUs)
        //     - generally, hide platform-specific bootloader stuff better


    }
}
