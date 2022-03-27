#![no_std]

use platypos_platform::Platform;

mod panic;

pub fn kmain<P: Platform>() -> ! {
    loop {}
}
