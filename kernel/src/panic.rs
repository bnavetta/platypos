use core::panic::PanicInfo;

use log::error;

#[cfg(not(test))]
#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);
    loop {}
}
