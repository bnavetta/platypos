#![no_std]
#![no_main]

mod panic;

#[export_name = "_start"]
extern "C" fn start() {
    loop {}
}
