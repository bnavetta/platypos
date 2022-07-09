//! The kernel heap allocator. This is the global allocator that the Rust
//! `alloc` crate expects.

use core::alloc::GlobalAlloc;
use core::mem::MaybeUninit;

use linked_list_allocator::LockedHeap;

struct KernelHeapAllocator {
    // TODO: whatever allocator implementation I go with can start with a static area and add more
    // dynamically (instead of special "early" allocator)
    inner: LockedHeap,
}

#[global_allocator]
static KERNEL_HEAP: KernelHeapAllocator = KernelHeapAllocator::new();

// Start with 32 KiB - the tracing infrastructure is kind of memory-hungry
static mut BUF: [MaybeUninit<u8>; 32768] = MaybeUninit::uninit_array();

/// Bootstrap the kernel keap allocator.
///
/// # Safety
/// This must be called exactly once, and before any allocations are made
pub unsafe fn init() {
    KERNEL_HEAP.inner.lock().init_from_slice(&mut BUF);
}

impl KernelHeapAllocator {
    const fn new() -> Self {
        let inner = LockedHeap::empty();
        Self { inner }
    }
}

unsafe impl GlobalAlloc for KernelHeapAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        self.inner.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.inner.dealloc(ptr, layout)
    }
}
