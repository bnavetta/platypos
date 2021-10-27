//! Early-boot memory allocator.
//! 
//! This module provides a fixed-size, single-threaded bump allocator for the earliest stages of booting, such as parsing the memory map.
//! As soon as the system is initialized, this allocator becomes read-only.

use core::alloc::{AllocError, Allocator, Layout};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

static mut DATA: [u8; 8 * 1024] = [0u8; 8 * 1024];
static OFFSET: AtomicUsize = AtomicUsize::new(0);
static LOCKED: AtomicBool = AtomicBool::new(false); 


pub struct EarlyAllocator;

pub const ALLOC: EarlyAllocator = EarlyAllocator;

unsafe impl Allocator for EarlyAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        use core::fmt::Write;

        assert!(!LOCKED.load(Ordering::Relaxed), "Early allocator is locked!");

        let offset = OFFSET.load(Ordering::Relaxed);
        let rem = offset % layout.align();
        let aligned_offset = if rem == 0 { offset } else { offset + layout.align() - rem };
        let next_offset = aligned_offset + layout.size();

        let _ = writeln!(log(), "Will allocate {} bytes at {} (aligned up from {})", layout.size(), aligned_offset, offset);

        if next_offset > unsafe { DATA.len() } {
            // Not enough memory to satisfy the allocation
            Err(AllocError)
        } else {
            let ptr = unsafe { DATA.as_mut_ptr().add(aligned_offset) };
            let _ = writeln!(log(), "Allocation pointer is {:p}", ptr);
            OFFSET.compare_exchange(offset, next_offset, Ordering::SeqCst, Ordering::Acquire).expect("Possible concurrent allocations");
            // Safety: ptr is within the bounds of DATA, and so non-null
            let base = unsafe { NonNull::new_unchecked(ptr) };
            Ok(NonNull::slice_from_raw_parts(base, layout.size()))
        }
    }

    unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
        // nothing to do
    }
}

/// Lock the early allocator. This prevents further allocations.
pub fn lock() {
    LOCKED.store(true, Ordering::Relaxed);
}

fn log() -> impl ::core::fmt::Write {
    unsafe { crate::driver::uart::Uart::new(0x1000_0000) }
}