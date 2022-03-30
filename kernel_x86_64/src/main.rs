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
