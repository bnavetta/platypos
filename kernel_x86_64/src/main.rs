#![no_std]
#![no_main]

use bootloader::{entry_point, BootInfo};
use platypos_kernel::{kmain, BootArgs, KernelLog};
use spin::Once;

mod framebuffer;
mod platform;

use self::framebuffer::FrameBufferTarget;
use self::platform::PlatformX86_64;

static LOG: Once<KernelLog<PlatformX86_64>> = Once::new();

fn start(info: &'static mut BootInfo) -> ! {
    let args = BootArgs {
        display: info.framebuffer.as_mut().map(FrameBufferTarget::new),
    };

    let log = LOG.call_once(|| {
        let mut serial = unsafe { uart_16550::SerialPort::new(0x3f8) };
        serial.init();
        KernelLog::new(serial)
    });
    log::set_logger(log).expect("logger already installed");
    log::set_max_level(log::LevelFilter::Trace);

    kmain::<PlatformX86_64>(args);
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
