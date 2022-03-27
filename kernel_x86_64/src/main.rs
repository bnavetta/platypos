#![no_std]
#![no_main]

use bootloader::{entry_point, BootInfo};
use platypos_kernel::kmain;

mod platform;

use self::platform::PlatformX86_64;

static HELLO: &[u8] = b"Hello World!";

fn start(_info: &'static mut BootInfo) -> ! {
    // let vga_buffer = 0xb8000 as *mut u8;

    // for (i, &byte) in HELLO.iter().enumerate() {
    //     unsafe {
    //         *vga_buffer.offset(i as isize * 2) = byte;
    //         *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
    //     }
    // }

    kmain::<PlatformX86_64>();
}

entry_point!(start);
