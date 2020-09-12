//! General-purpose memory allocation

use core::alloc::{GlobalAlloc, Layout};
use core::marker::PhantomData;

use platypos_pal as pal;

pub struct SlabAllocator<P: pal::Platform> {
    _platform: PhantomData<&'static P>
}

impl <P: pal::Platform> SlabAllocator<P> {
    pub const fn new() -> SlabAllocator<P> {
        SlabAllocator { _platform: PhantomData }
    }
}

unsafe impl <P: pal::Platform> GlobalAlloc for SlabAllocator<P> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unimplemented!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unimplemented!()
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unimplemented!()
    }
}

#[alloc_error_handler]
fn handle_alloc_error(layout: Layout) -> ! {
    panic!("Allocation failed for {:?}", layout)
}