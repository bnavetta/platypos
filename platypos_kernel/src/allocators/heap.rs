use core::alloc::{GlobalAlloc, Layout};

pub struct HeapAllocator;

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unimplemented!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!()
    }
}

impl HeapAllocator {
    pub const fn new() -> HeapAllocator {
        HeapAllocator
    }
}