//! Physical memory management

use bitvec::prelude::*;
use fixed_slice_vec::FixedSliceVec;
use x86_64::{PhysAddr, structures::paging::{PageSize, PhysFrame, Size4KiB}};

// Overall approach:
// * Only supporting newer hardware, so don't worry about arbitrarily-sized allocations
// * Maintain a bitmap (via https://github.com/bitvecto-rs/bitvec) that's the source of truth for whether or not a given frame is allocated
// * For each page size (4KiB, 2MiB, 1GiB), keep a fixed-capacity stack of free pages of that size
//   * When the stack is exhausted, scan the bitmap to repopulate it (note: huge pages have to be aligned based on the page size, so we can scan via chunks to populate their stack)
// * How to allocate:
//   * pop from the stack
//   * if stack is empty, scan the bitmap for free pages (can do this in a background task once that's a thing)
//   * mark as allocated in the bitmap
// * How to free:
//   * if stack isn't already full, push onto the stack
//   * mark as unallocated in the bitmap
// * Initialization:
//   * Figure out extent of physical memory
//   * Find a contiguous region of physical memory big enough to hold the bitmap and stacks
//   * Map this region into memory
//       - TODO: how to do this, given that we might need to allocate for the page table?
//       - could reserve a couple extra pages in addition to the bitmap+stacks, and use that for extra page table memory if needed
//   * Initialize the bitmap based on the bootloader memory map
//   * Initialize the stacks based on the bitmap
//
// With no linear mapping of all physical memory, how do we access arbitrary physical pages as needed?
// * Fully fill out the page tables for a chunk of the kernel's address space (these can all point to frame 0 initially) - done in the bootloader
// * To access a page of physical memory, map it into this region - we know it can be done without allocating, because the page tables were preallocated!
// * Can use a bitvec to track usage (or KISS - lease out the entire working set at once, use a counter, reset when you're done)


// TODO: wrap spinlock to also handle interrupt state

/// Physical memory manager. The `MemoryManager` is responsible for allocating and deallocating frames of physical memory.
pub struct MemoryManager {
    /// Bitvec tracking whether or not a given frame is allocated. In this implementation, `0` means a frame is unallocated/free and `1` means that it is allocated.
    allocations: &'static mut BitSlice,

    free_stack_4kib: FixedSliceVec<'static, PhysFrame>,
    // TODO: how to handle overlapping lists... or just don't support multiple sizes?
    free_stack_2mib: FixedSliceVec<'static, PhysFrame>,
    free_stack_1gib: FixedSliceVec<'static, PhysFrame>,
}

impl MemoryManager {
    #[inline]
    fn is_allocated(&self, frame: PhysFrame) -> bool {
        let index = frame.start_address().as_u64() / frame.size();
        self.allocations[index as usize]
    }

    #[inline]
    fn set_allocated(&mut self, frame: PhysFrame, allocated: bool) {
        let index = frame.start_address().as_u64() / frame.size();
        self.allocations.set(index as usize, allocated);
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame> {
        if self.free_stack_4kib.is_empty() {
            self.scan_free_frames();
        }
        let frame = self.free_stack_4kib.pop()?;
        self.set_allocated(frame, true);
        Some(frame)
    }

    fn scan_free_frames(&mut self) {
        let mut free_frames = self.allocations.iter_zeros();
        for _ in self.free_stack_4kib.len()..self.free_stack_4kib.capacity() {
            match free_frames.next() {
                Some(frame) => {
                    let addr = PhysAddr::new(frame as u64 * Size4KiB::SIZE);
                    // Safety: addr is calculated by multiplying by the page size, so it's guaranteed to be aligned
                    self.free_stack_4kib.push(unsafe { PhysFrame::from_start_address_unchecked(addr) });
                },
                None => break,
            }
        }
    }
}