use core::panic::PanicInfo;

use log::error;

use crate::qemu;

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);

    if cfg!(test) {
        qemu::exit(qemu::ExitCode::Failure);
    }

    loop {
        x86_64::instructions::hlt();
    }
}
