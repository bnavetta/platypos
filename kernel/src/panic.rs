use core::panic::PanicInfo;

use log::error;

use crate::{qemu, util::hlt_loop};

#[panic_handler]
pub fn panic(info: &PanicInfo) -> ! {
    error!("{}", info);

    if cfg!(test) {
        qemu::exit(qemu::ExitCode::Failure);
    } else {
        hlt_loop();
    }
}
