use core::{
    alloc::{GlobalAlloc, Layout},
    ptr,
};

use log::{info, trace};
use spin::{Mutex, Once};
use x86_64::VirtAddr;
use crate::memory::allocator::MemoryAllocator;
use crate::memory::frame::FrameAllocator;

pub mod allocator;
pub mod frame;
pub mod page_table;

pub const FRAME_SIZE: usize = 4096;

pub const HEAP_START: u64 = 0xfffffbbbbbbb0000;
pub const HEAP_END: u64 = 0xfffffbbcbbbb0000; // 4GiB

/// Very simple bump allocator for any allocations that are made while initializing the real
/// allocator. Does not support deallocation
#[derive(Debug)]
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
            trace!(
                "Allocating {} bytes from bootstrap region ({} bytes remaining)",
                layout.size(),
                self.heap_end.as_u64() - new_end
            );
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

pub fn bootstrap_allocator(allocator: &FrameAllocator) {
    let bootstrap_heap = allocator
        .allocate_pages(2)
        .expect("Could not allocate bootstrap heap");
    let heap_start = VirtAddr::from_ptr(bootstrap_heap);
    let heap_end = heap_start + 2 * FRAME_SIZE as u64;

    info!(
        "Bootstrap heap starting at {:#x} and ending at {:#x}",
        heap_start.as_u64(),
        heap_end.as_u64()
    );
    REAL_ALLOCATOR.call_once(|| {
        Mutex::new(AllocatorMode::Bootstrap(BootstrapAllocator::new(
            heap_start, heap_end,
        )))
    });
}

pub fn initialize_allocator() {
    info!("Switching over to main heap in {:#x}-{:#x}", HEAP_START, HEAP_END);

    let mut mode = REAL_ALLOCATOR.wait().expect("Allocator not bootstrapped").lock();

    let allocator = match &*mode {
        &AllocatorMode::Bootstrap(ref allocator) => MemoryAllocator::new(VirtAddr::new(HEAP_START), VirtAddr::new(HEAP_END), allocator.heap_start, allocator.heap_end),
        &AllocatorMode::Initialized(_) => panic!("Allocator already initialized")
    };

    *mode = AllocatorMode::Initialized(allocator);
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

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
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
