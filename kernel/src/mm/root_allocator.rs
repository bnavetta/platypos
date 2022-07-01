//! The "root" kernel allocator. This allocator is responsible for managing
//! physical memory, and so it underpins all other allocators in the system.
//!
//! Unlike most other kernel physical memory allocators, the root allocator does
//! not allocate individual page frames. Instead, it uses fixed-size blocks that
//! represent the largest possible single allocation. Other allocators stacked
//! on top of the root allocator can manage individual page frames. This means
//! that large buffers are still available (e.g. for DMA) while keeping the root
//! allocator simpler. In addition, it's generally better for other allocators
//! to be fed large blocks to reduce the amount of bookkeeping they need to do.

use alloc::vec::Vec;

use linked_list_allocator::LockedHeap;
use static_assertions::const_assert;

use crate::arch::mm::MemoryAccess;
use crate::prelude::*;

use super::map::Region;

/// Builder for the physical memory allocator
pub struct Builder;

/// Size in page frames of the scratch region to allocate for bookkeeping while
/// configuring the allocator
const SCRATCH_PAGES: usize = 2;

/// Size of blocks allocated by the root.
pub const BLOCK_SIZE: usize = 2 * 1024 * 1024;
const PAGES_PER_BLOCK: usize = BLOCK_SIZE / PAGE_SIZE;

// On all platforms, the block size must be a multiple of the page size
const_assert!(BLOCK_SIZE % PAGE_SIZE == 0);

impl Builder {
    pub fn parse_memory_map<I>(
        &mut self,
        access: &mut MemoryAccess,
        mut memory_map: I,
        reserved: &[PageFrameRange],
    ) -> Result<(), Error>
    where
        I: Iterator<Item = Region> + Clone,
    {
        log::info!("Initializing physical memory allocator");

        // First, find a scratch region:
        let scratch = memory_map
            .clone()
            .find_map(|r| {
                if r.usable() && r.size() >= (SCRATCH_PAGES * PAGE_SIZE) {
                    for reserved_region in reserved.iter() {
                        if reserved_region.address_range().intersects(&r.range()) {
                            return None;
                        }
                    }

                    let start_pf = PageFrame::from_start(r.start())
                        .expect("Memory region is not page-aligned!");

                    Some(PageFrameRange::from_start_size(start_pf, SCRATCH_PAGES))
                } else {
                    None
                }
            })
            .ok_or(Error::new(ErrorKind::InsufficientMemory))?;

        log::debug!("Scratch space: {}", scratch.address_range());

        unsafe {
            access.with_memory::<_, Result<(), Error>>(scratch, |s| {
                // LockedHeap expects that the passed-in region is 'static, but the allocator
                // and allocations made with it don't escape this block, so we should be ok.
                let alloc = LockedHeap::new(s.as_mut_ptr().cast(), s.len());

                let mut ranges = Vec::new_in(&alloc);

                for region in memory_map {
                    if region.usable() {
                        let start = PageFrame::from_start(region.start())
                            .expect("Memory region is not page-aligned!");
                        assert!(
                            region.size() % PAGE_SIZE == 0,
                            "Region size is not a whole number of pages!"
                        );
                        let size = region.size() / PAGE_SIZE;
                        ranges.push(PageFrameRange::from_start_size(start, size));
                    }
                }

                ranges.shrink_to_fit();
                ranges.sort_unstable_by_key(|r| r.start());

                // Combine overlapping memory ranges
                // Look! The interview question came in handy :P
                let mut i = 1; // We know there's at least one usable region
                while i < ranges.len() {
                    let prev = &ranges[i - 1];
                    let cur = &ranges[i];
                    if prev.intersects(cur) {
                        let new_size = cur.end() - prev.start();
                        ranges.remove(i);
                        // Re-borrow mutably here to avoid a split borrow
                        (&mut ranges[i - 1]).set_size(new_size);
                    } else {
                        i += 1;
                    }
                }

                // TODO: remove reserved ranges

                log::debug!("Usable memory:");
                for range in &ranges {
                    log::debug!(" - {}", range.address_range());
                }

                // Ending page frame number is the number of page frames we have to track
                let page_frames = ranges.last().unwrap().end().as_usize();
                let bytes_needed = (page_frames / PAGES_PER_BLOCK).div_ceil(u8::BITS as usize);

                let bitmap_location = ranges
                    .iter()
                    .find_map(|r| {
                        if r.size_bytes() >= bytes_needed {
                            Some(PageFrameRange::from_start_size(
                                r.start(),
                                bytes_needed.div_ceil(PAGE_SIZE),
                            ))
                        } else {
                            None
                        }
                    })
                    .ok_or(Error::new(ErrorKind::InsufficientMemory))?;

                log::debug!("Placing bitmap at {}", bitmap_location.address_range());

                // Block approach has a big fragmentation problem:
                // - can't use small usable regions < 2MiB
                // - annoying to use regions that don't start on a 2MiB boundary

                // Solution?
                // - allocator manages individual pages
                // - scan bitmap to find big chunks, pull those out, and put them in a separate
                //   free list
                // - can always add big chunks back to pool if needed
                // LK (used in Fuschia) finds contiguous runs as needed: https://github.com/littlekernel/lk/blob/master/kernel/vm/pmm.c#L283
                // But Fuschia does something more complicated? https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/kernel/phys/lib/memalloc/pool-test.cc
                // Linked list of runs of free pages!?
                // https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/kernel/phys/lib/memalloc/include/lib/memalloc/pool.h

                drop(ranges);
                drop(alloc);

                Ok(())
            })??;
        }

        /*

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
                    // Safety: all usable memory is unallocated at this point, so there
                    // is not an existing mapping to alias TODO: better PPN
                    // type

                    //let addr = access.map_permanent(todo!("PPN
                    // API"), bitmap_size.div_ceil(PAGE_SIZE))?; TODO: make
                    // sure we have a whole-usize allocation - may need to
                    // round up bitmap_size for that
                };

        */

        Ok(())

        // TODOs:
        // - API for temporarily accessing physical memory (could return a &'a
        //   [MaybeUninit<u8>] to ensure reference doesn't escape)
        // - alternative construction algorithm to handle unusable memory like
        //   kernel: 1. Find a usable page frame (in list from bootloader,
        //   doesn't overlap w/ reserved regions) 2. Use that to create a
        //   scratch allocator 3. Put memory into a Vec in scratch allocator -
        //   then can sort, coalesce, split out reserved regions
        // - clean up xtasks:
        //     - support building multiple platforms at once
        //     - more composable abstractions:
        //         - build crate X for platform Y, producing the binary artifact
        //         - run kernel-like artifact (platform-appropriate) on platform
        //           Y (platform knows which qemu to use, can pass generic
        //           settings like memory+CPUs)
        //     - generally, hide platform-specific bootloader stuff better
    }
}
