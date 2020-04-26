use core::panic::PanicInfo;

#[cfg(target_arch = "x86_64")]
#[panic_handler]
pub fn handle_panic(_info: &PanicInfo) -> ! {
    x86_64_ext::instructions::hlt_loop();
}
