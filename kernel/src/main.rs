#![feature(panic_implementation)]
#![no_std]
#![no_main]

extern crate bootloader_precompiled;

#[macro_use]
extern crate dbg;

use core::panic::PanicInfo;

use dbg::Category;

static HELLO: &[u8] = b"Hello World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    dbg::init(0x3F8);

    dbg!(Mode::Error, "Hello, World!");

    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}

#[panic_implementation]
#[no_mangle]
pub fn panic(_info: &PanicInfo) -> ! {
    loop {}
}