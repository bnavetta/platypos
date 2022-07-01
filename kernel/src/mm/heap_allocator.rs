//! The kernel heap allocator. This is the global allocator that the Rust
//! `alloc` crate expects.

use core::alloc::GlobalAlloc;

struct KernelHeapAllocator {}

#[global_allocator]
static KERNEL_HEAP: KernelHeapAllocator = KernelHeapAllocator::new();

impl KernelHeapAllocator {
    const fn new() -> Self {
        Self {}
    }
}

unsafe impl GlobalAlloc for KernelHeapAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        todo!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        todo!()
    }
}
