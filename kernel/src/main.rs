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

use tracing::info;

use driver::uart::Uart;
use sys::devicetree::DeviceTree;

// #[cfg_attr(target_arch="riscv", path="arch/riscv.rs")]
#[path = "arch/riscv/mod.rs"]
mod arch;

mod alloc;
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

    /*
    let devtree = unsafe {

        DevTree::from_raw_pointer(fdt_addr).unwrap()
    };

    let _ = writeln!(&mut driver, "DeviceTree version: {}", devtree.version());

    let mut iter = devtree.items();
    while let Some(item) = iter.next().unwrap() {
        match item {
            DevTreeItem::Node(n) => {
                let _ = writeln!(&mut driver, "{}:", n.name().unwrap());
                let mut props = n.props();
                while let Some(prop) = props.next().unwrap() {
                    write_prop(&mut driver, &prop, "  ");
                }
            },
            DevTreeItem::Prop(p) => {
                write_prop(&mut driver, &p, "");
            },
        }
    }
    */

    arch::abort();
}

/*

fn write_prop(w: &mut Uart, prop: &DevTreeProp, indent: &str) {
    let _ = write!(w, "{}{} = ", indent, prop.name().unwrap());
    if prop.length() == 0 {
        let _ = write!(w, "<empty>");
    } else if all_strings(prop.iter_str()) {
        let mut iter = prop.iter_str();
        while let Some(s) = iter.next().unwrap() {
            let _ = write!(w, "\"{}\" ", s);
        }
    } else if prop.length() % ::core::mem::size_of::<u32>() == 0 {
        for val in prop.propbuf().array_chunks::<4>() {
            let v = u32::from_be_bytes(*val);
            let _ = write!(w, "{:#010x} ", v);
        }
    } else {
        let _ = write!(w, "<");
        for val in prop.propbuf() {
            let _ = write!(w, "{:02x} ", val);
        }
        let _ = write!(w, ">");
    }

    let _ = writeln!(w);
}

fn all_strings(mut it: StringPropIter) -> bool {
    loop {
        match it.next() {
            Ok(Some(s)) => if s.is_empty() { return false },
            Ok(None) => return true,
            Err(_) => return false
        }
    }
}

*/

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    log::error!("PANIC: {}", info);
    arch::abort();
}
