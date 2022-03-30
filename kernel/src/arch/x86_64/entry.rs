//! Entry point for x86_64 systems

use bootloader::{entry_point, BootInfo};

use crate::BootArgs;

use super::display::FrameBufferTarget;

/// Entry point called by the bootloader
fn start(info: &'static mut BootInfo) -> ! {
    let args = BootArgs {
        display: info.framebuffer.as_mut().map(FrameBufferTarget::new),
    };

    let mut serial = unsafe { uart_16550::SerialPort::new(0x3f8) };
    serial.init();
    crate::logging::init(serial);

    crate::kmain(args);
}

entry_point!(start);

/*

Physical memory management
- in practice, just allocating pages is enough - don't bother with 2^n page allocations / buddies
- use a fixed-size stack of free frames for speed, plus a bitmap to hold remaining frame state
- frames that are on the stack are marked as allocated in the bitmap
- can mark memory holes as allocated in the bitmap also, and/or make the bitmap an array of regions (can abstract that away)
- if stack is empty, scan bitmap for free pages to refill it

Physical memory access
- abstract away whether physical memory is mapped into the kernel address space
- two APIs:
   - permanently map a chunk of physical memory (needed to create bitmaps for physical memory manager)
     - infinite recursion risk if this needs to allocate frames to create the virtual memory mappings
     - have parameter that specifies how to allocate page frames:
       - normally, ask physical memory manager for them
       - if you _are_ the physical memory manager, provide a preallocated buffer of frames
   - temporarily map a chunk of physical memory using a RAII guard
     - this could just use an existing mapping, or map it into a reserved chunk of address space
- platform_common should provide a default implementation for platforms where all physical memory is mapped

Other stuff:
- x86_64 bootloader crate (and probably other platforms) can pass TLS (thread-local storage) info to kernel
- see if that can be repurposed as CPU-local storage (need to figure out how it's accessed)
- use thingbuf to send info from interrupt handlers to regular (or high-priority even) tasks

*/
