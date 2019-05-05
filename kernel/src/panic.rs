use core::panic::PanicInfo;

use dbg::{Category, dbg};

#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    dbg!(Category::Error, "{}", info);
    loop {}
}
