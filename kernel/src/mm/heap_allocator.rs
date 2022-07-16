//! The kernel heap allocator. This is the global allocator that the Rust
//! `alloc` crate expects.

use core::alloc::GlobalAlloc;
use core::mem::MaybeUninit;

use crate::mm::root_allocator::Allocator as RootAllocator;
use crate::prelude::*;
use alloc::sync::Arc;
use platypos_ktrace::if_not_tracing;

use linked_list_allocator::LockedHeap;
use spin::Once;

struct KernelHeapAllocator {
    // TODO: whatever allocator implementation I go with can start with a static area and add more
    // dynamically (instead of special "early" allocator)
    inner: LockedHeap,
    root: Once<Arc<RootAllocator<'static>>>,
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

/// Provide the root memory allocator after it's been initialized, enabling the
/// heap to grow.
pub fn enable_expansion(root: Arc<RootAllocator<'static>>) {
    KERNEL_HEAP.root.call_once(|| root);
}

impl KernelHeapAllocator {
    const fn new() -> Self {
        let inner = LockedHeap::empty();
        Self {
            inner,
            root: Once::INIT,
        }
    }
}

unsafe impl GlobalAlloc for KernelHeapAllocator {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let _trace = if_not_tracing!(tracing::trace_span!("alloc", size = layout.size()));
        let res = self.inner.alloc(layout);
        if res.is_null() {
            if_not_tracing!(tracing::warn!("allocation failed"));
        } else {
            if_not_tracing!(tracing::trace!(vaddr = res.addr(), "allocation succeeded"));
        }
        res
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        let _trace = if_not_tracing!(tracing::trace_span!(
            "dealloc",
            size = layout.size(),
            vaddr = ptr.addr()
        ));
        self.inner.dealloc(ptr, layout)
    }
}
