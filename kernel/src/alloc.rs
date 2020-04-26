use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use spinning_top::Spinlock;

// For now, fixed-size bump allocator to unblock using slog (which relies heavily on Arc)
struct KernelAllocator {
    inner: Spinlock<AllocatorInner>
}

impl KernelAllocator {
    pub const fn new() -> KernelAllocator {
        KernelAllocator {
            inner: Spinlock::new(AllocatorInner {
                heap: [0; 1024],
                index: 0
            })
        }
    }
}

struct AllocatorInner {
    heap: [u8; 1024],
    index: usize
}

#[global_allocator]
static KERNEL_ALLOCATOR: KernelAllocator = KernelAllocator::new();

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut inner = self.inner.lock();
        let padding = layout.align() - (inner.index % layout.align());
        let start = inner.index + padding;
        let new_index = start + layout.size();
        if new_index > inner.heap.len() {
            ptr::null_mut()
        } else {
            inner.index = new_index;
            &mut inner.heap[start] as *mut u8
        }
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        // freeing not supported
    }
}

#[alloc_error_handler]
fn handle_alloc_error(layout: Layout) -> ! {
    panic!("Allocating {:?} failed", layout);
}