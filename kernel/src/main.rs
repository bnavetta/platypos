#![no_std]
#![no_main]
#![feature(alloc_error_handler, allocator_api, asm, const_ptr_offset, maybe_uninit_array_assume_init, maybe_uninit_uninit_array, nonnull_slice_from_raw_parts, slice_ptr_get)]

// extern crate alloc;

use platypos_boot_info::BootInfo;
use tracing::info;
use x86_64::instructions::hlt;

mod memory;
mod trace;
mod util;

#[no_mangle]
pub extern "C" fn _start(boot_info: &'static BootInfo) -> ! {
    wait_for_debugger();
    trace::init();
    kernel_main(boot_info);
    loop {
        hlt();
    }
}

#[tracing::instrument]
fn kernel_main(boot_info: &'static BootInfo) {
    boot_info.assert_valid();
    info!(%boot_info, "Boot info address: {:#p}", boot_info);
}

/// The GDB setup script will set this to 1 after it's attached
#[cfg(feature = "gdb")]
static mut KERNEL_DEBUGGER_ATTACHED: u8 = 0;

#[cfg(feature = "gdb")]
fn wait_for_debugger() {
    unsafe {
        while KERNEL_DEBUGGER_ATTACHED == 0 {
            asm!("pause", options(nomem, nostack, preserves_flags));
        }
    }
}

#[cfg(not(feature = "gdb"))]
fn wait_for_debugger() {}
