//! Memory allocators

use core::alloc::{Allocator, GlobalAlloc, Layout};
use core::ptr::{
    NonNull, {self},
};

use log::error;

pub mod early;
pub mod physical;

pub struct KernelAllocator;

#[global_allocator]
static KERNEL_ALLOC: KernelAllocator = KernelAllocator;

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match early::ALLOC.allocate(layout) {
            Ok(ptr) => ptr.as_mut_ptr(),
            Err(_) => ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let non_null = NonNull::new(ptr).expect("dealloc() called with null pointer");
        early::ALLOC.deallocate(non_null, layout)
    }
}

#[alloc_error_handler]
fn allocation_error(layout: Layout) -> ! {
    error!("Allocation failure for {:?}", layout);
    crate::arch::abort()
}
