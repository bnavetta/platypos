#![no_std]
#![no_main]

use bootloader::{entry_point, BootInfo};
use platypos_kernel::{kmain, BootArgs};

mod framebuffer;
mod platform;

use self::framebuffer::FrameBufferTarget;
use self::platform::PlatformX86_64;

fn start(info: &'static mut BootInfo) -> ! {
    let args = BootArgs {
        display: info.framebuffer.as_mut().map(FrameBufferTarget::new),
    };

    kmain::<PlatformX86_64>(args);
}

entry_point!(start);
