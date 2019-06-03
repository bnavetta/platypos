use core::panic::PanicInfo;

use log::error;

use crate::util::{hlt_loop, qemu};

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);

    if cfg!(test) {
        qemu::exit(qemu::ExitCode::Failure);
    } else {
        hlt_loop();
    }
}
