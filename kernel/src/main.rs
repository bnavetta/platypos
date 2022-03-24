#![no_std]
#![no_main]
#![feature(
    panic_info_message,
    alloc_error_handler,
    allocator_api,
    asm,
    global_asm,
    int_roundings,
    array_chunks,
    maybe_uninit_slice,
    nonnull_slice_from_raw_parts,
    ptr_as_uninit,
    slice_ptr_get,
    slice_ptr_len
)]

extern crate alloc;

use tracing::info;

use driver::uart::Uart;
use sys::devicetree::DeviceTree;

// #[cfg_attr(target_arch="riscv", path="arch/riscv.rs")]
#[path = "arch/riscv/mod.rs"]
mod arch;

mod allocator;
mod diagnostic;
mod driver;
mod sys;

#[no_mangle]
extern "C" fn kmain(hart_id: usize, fdt_addr: *const u8) -> ! {
    diagnostic::init();

    info!("Welcome to PlatypOS!");

    info!(
        boot_hart = hart_id,
        fdt_addr = fdt_addr as usize,
        "Initializing system..."
    );

    let device_tree = unsafe { DeviceTree::new(fdt_addr) };

    if let Some(serial_port) = device_tree.find_serial_port() {
        let uart = unsafe { Uart::new(serial_port) };
        diagnostic::enable_serial(uart);
    } else {
        panic!("Serial console unavailable");
    }

    device_tree.info();

    let memory_map = device_tree.memory_map();
    info!("Memory map:\n{}", memory_map);
    let (alloc_start, alloc_end) = memory_map.allocatable_ram_range();
    info!("Allocatable RAM: {} - {}", alloc_start, alloc_end);

    let phys_allocator = allocator::physical::initialize_allocator(&memory_map);

    arch::abort();
}

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("PANIC: {}", info);
    arch::abort();
}
