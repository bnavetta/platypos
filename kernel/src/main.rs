#![no_std]
#![no_main]

use core::panic::PanicInfo;

use x86_64::instructions::hlt;

static mut foo: &str = "Hello, World!";

#[no_mangle]
pub extern "C" fn _start() -> ! {
    loop {
        hlt();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}
