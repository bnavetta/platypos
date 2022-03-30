use core::panic::PanicInfo;

use crate::arch::interrupts;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        interrupts::halt_until_interrupted()
    }
}
