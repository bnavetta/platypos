use core::alloc::{GlobalAlloc, Layout};
use core::ptr;

use spin::{Mutex, Once};
use x86_64::VirtAddr;

pub mod allocator;
pub mod frame;
pub mod page_table;

struct BootstrapAllocator {
    heap_start: VirtAddr,
    heap_end: VirtAddr,
    current: VirtAddr,
}

impl BootstrapAllocator {
    fn new(heap_start: VirtAddr, heap_end: VirtAddr) -> BootstrapAllocator {
        BootstrapAllocator {
            heap_start,
            heap_end,
            current: heap_start,
        }
    }

    fn alloc(&mut self, layout: Layout) -> *mut u8 {
        let start = x86_64::align_up(self.current.as_u64(), layout.align() as u64);
        let new_end = start + layout.size() as u64;
        if new_end > self.heap_end.as_u64() {
            ptr::null_mut()
        } else {
            self.current = VirtAddr::new(new_end);
            VirtAddr::new(start).as_mut_ptr()
        }
    }
}

enum AllocatorMode {
    Bootstrap(BootstrapAllocator),
    Initialized(allocator::MemoryAllocator),
}

// Indirection with an empty KernelAllocator struct because #[global_allocator] has to be a static which
// directly implements GlobalAlloc
static REAL_ALLOCATOR: Once<Mutex<AllocatorMode>> = Once::new();

pub fn bootstrap_allocator(heap_start: VirtAddr, heap_end: VirtAddr) {
    REAL_ALLOCATOR.call_once(|| {
        Mutex::new(AllocatorMode::Bootstrap(BootstrapAllocator::new(
            heap_start, heap_end,
        )))
    });
}

pub struct KernelAllocator;

impl KernelAllocator {
    pub const fn new() -> KernelAllocator {
        KernelAllocator
    }
}

unsafe impl GlobalAlloc for KernelAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let mut mode = REAL_ALLOCATOR
            .wait()
            .expect("Allocator not bootstrapped")
            .lock();

        match &mut *mode {
            &mut AllocatorMode::Bootstrap(ref mut allocator) => allocator.alloc(layout),
            &mut AllocatorMode::Initialized(ref mut allocator) => {
                allocator.allocate(layout.size()).unwrap_or(ptr::null_mut())
            }
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        let mut mode = REAL_ALLOCATOR
            .wait()
            .expect("Allocator not bootstrapped")
            .lock();

        match &mut *mode {
            &mut AllocatorMode::Bootstrap(_) => (),
            &mut AllocatorMode::Initialized(ref mut allocator) => allocator.free(ptr),
        }
    }
}
