use core::panic::PanicInfo;

#[cfg(not(test))]
#[panic_handler]
pub fn handle_panic(info: &PanicInfo) -> ! {
    loop {}
}