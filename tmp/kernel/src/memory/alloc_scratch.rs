//! Kernel memory allocators

use core::alloc::{Allocator, AllocError, Layout};
use core::ptr::{self, NonNull};

use paste::paste;
use spinning_top::Spinlock;
use x86_64::instructions::interrupts::without_interrupts;

pub struct StaticPool {
    allocator: Spinlock<FixedBumpAllocator>,
}

#[macro_export]
macro_rules! static_pool {
    ($name:ident, $capacity:expr) => {
        paste::paste! {
            static [<$name _STORAGE>]: [u8; $capacity] = [0u8; $capacity];

            static $name: $crate::memory::alloc::StaticPool = unsafe { $crate::memory::alloc::StaticPool::new(::core::ptr::NonNull::new_unchecked([<$name _STORAGE>].as_ptr() as *mut u8), $capacity) };
        }
    };
}

/// A bump allocator backed by a fixed-size pool of memory.
struct FixedBumpAllocator {
    pool_start: NonNull<u8>,
    pool_end: NonNull<u8>,
    bump_pointer: NonNull<u8>,
}

unsafe impl <'a> Allocator for &'a StaticPool {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        self.with_allocator(|alloc| {
            unsafe { alloc.try_allocate(layout) }.map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
        })
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // nothing to do
    }
}

impl StaticPool {
    pub const unsafe fn new(start: NonNull<u8>, capacity: usize) -> StaticPool {
        let end = NonNull::new_unchecked(start.as_ptr().add(capacity));
        let allocator = FixedBumpAllocator {
            pool_start: start,
            // Safety: non-null + positive offset = non-null (although overflow is possible...)
            pool_end: end,
            bump_pointer: end,
        };
        StaticPool {
            allocator: Spinlock::new(allocator),
        }
    }

    fn with_allocator<F, T>(&self, f: F) -> T where F: FnOnce(&mut FixedBumpAllocator) -> T {
        without_interrupts(|| {
            let mut allocator = self.allocator.lock();
            f(&mut allocator)
        })
    }
}

impl FixedBumpAllocator {
    #[inline]
    unsafe fn try_allocate(&mut self, layout: Layout) -> Result<NonNull<u8>, AllocError> {
        // Bump down from the start of the memory pool - this is more efficient
        // See https://fitzgeraldnick.com/2019/11/01/always-bump-downwards.html
        let ptr = self.bump_pointer.as_ptr() as usize;
        let ptr = ptr.checked_sub(layout.size()).ok_or(AllocError)?;
        let ptr = ptr & !(layout.align() - 1);

        let start = self.pool_start.as_ptr() as usize;
        if ptr < start {
            return Err(AllocError);
        }

        self.bump_pointer = NonNull::new(ptr as *mut u8).unwrap();
        Ok(self.bump_pointer)
    }

    #[inline]
    unsafe fn try_reallocate(&mut self, allocation: NonNull<u8>, old_layout: Layout, new_layout: Layout) -> Result<NonNull<u8>, AllocError> {
        if allocation == self.bump_pointer {
            // Efficient path: we can extend the existing allocation
            let extra_size = new_layout.size() - old_layout.size();
            let ptr = allocation.as_ptr() as usize;
            let ptr = ptr.checked_sub(extra_size).ok_or(AllocError)?;
            let ptr = ptr & !(new_layout.align() - 1);

            let start = self.pool_start.as_ptr() as usize;
            if ptr < start {
                return Err(AllocError);
            }
            let ptr = ptr as *mut u8;

            ptr::copy(allocation.as_ptr(), ptr, old_layout.size());

            self.bump_pointer = NonNull::new(ptr).unwrap();
            Ok(self.bump_pointer)
        } else {
            // Sad path: we have to allocate more memory
            let new_pointer = self.try_allocate(new_layout)?;
            // We can use copy_nonoverlapping because the new allocation is guaranteed not to overlap with the old one
            ptr::copy_nonoverlapping(allocation.as_ptr(), new_pointer.as_ptr(), old_layout.size());
            // No point freeing the old allocation since this is a bump allocator
            Ok(new_pointer)
        }
    }
}

// unsafe impl Allocator for FixedBumpAllocator {
//     fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
//         unsafe {
//             self.try_allocate(layout)
//             .map(|ptr| NonNull::slice_from_raw_parts(ptr, layout.size()))
//         }
//     }

//     unsafe fn deallocate(&self, _ptr: NonNull<u8>, _layout: Layout) {
//         // Nothing to do here
//     }
// }



// TODO: refactor debugging/logging/tracing into 2 layers:
// * `Logger` - emit formatted messages, no allocations, interrupt-safe
// * tracing - rich trace/span data, allocations?, interrupt-safe
//
// Add an `emit` method to `Logger`:
fn emit<F>(&mut self, level: tracing::Level, target: &str, f: F) where F: FnOnce(impl core::fmt::Write) -> core::fmt::Result {
    todo!()
}
// Then, use this to generate panic messages, OOM errors, backtraces, but also span/event logging
// the callback is invoked after printing the message prefix, and generates the actual message (could be a string, backtrace, event fields, etc.)

// Could also use an open-addressing fixed-size hashmap for span metadata

// Overall kernel setup
// - can't really avoid global variables because of interrupt handlers
//    - but maybe try to bundle it all in one place, that's set after the kernel is set up?
// - during initialization, try to pass dependencies explicitly to not accidentally rely on unrelated functions being called in the right order