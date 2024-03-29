//! The kernel heap allocator. This is the global allocator that the Rust
//! `alloc` crate expects.

use core::alloc::GlobalAlloc;
use core::mem::MaybeUninit;

use crate::mm::root_allocator::Allocator as RootAllocator;
use crate::prelude::*;
use platypos_common::sync::Global;

use linked_list_allocator::LockedHeap;

struct KernelHeapAllocator {
    // TODO: whatever allocator implementation I go with can start with a static area and add more
    // dynamically (instead of special "early" allocator)
    inner: LockedHeap,
    root: Global<&'static RootAllocator<'static>>,
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
pub fn enable_expansion(root: &'static RootAllocator<'static>) {
    KERNEL_HEAP.root.init(root);
}

impl KernelHeapAllocator {
    const fn new() -> Self {
        let inner = LockedHeap::empty();
        Self {
            inner,
            root: Global::new(),
        }
    }
}

unsafe impl GlobalAlloc for KernelHeapAllocator {
    #[tracing::instrument(level = "trace", skip_all, fields(size = layout.size()))]
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        let res = self.inner.alloc(layout);
        if res.is_null() {
            tracing::warn!("allocation failed");
        } else {
            tracing::trace!(vaddr = res.addr(), "allocation succeeded");
        }
        res
    }

    #[tracing::instrument(level = "trace", skip_all, fields(size = layout.size(), vaddr = ptr.addr()))]
    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        self.inner.dealloc(ptr, layout)
    }
}
