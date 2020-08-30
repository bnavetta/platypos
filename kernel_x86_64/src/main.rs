#![no_std]
#![no_main]

extern crate rlibc;

use core::panic::PanicInfo;

#[export_name = "_start"]
extern "C" fn start() {
    loop {}
}

#[panic_handler]
fn handle_panic(info: &PanicInfo) -> ! {
    loop {

    }
}