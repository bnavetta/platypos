#![no_std]
#![no_main]
#![feature(panic_info_message, allocator_api, asm, global_asm, array_chunks, nonnull_slice_from_raw_parts)]

use core::fmt::Write;

use driver::uart::Uart;
use sys::devicetree::DeviceTree;


// #[cfg_attr(target_arch="riscv", path="arch/riscv.rs")]
#[path ="arch/riscv.rs"]
mod arch;

mod alloc;
mod driver;
mod sys;

#[no_mangle]
extern "C" fn kmain(hart_id: usize, fdt_addr: *const u8) -> ! {
    let mut serial = unsafe {
        let mut uart = Uart::new(0x1000_0000);
        uart.init();
        uart
    };

    let _ = writeln!(&mut serial, "Hello, World!");
    let _ = writeln!(&mut serial, "hart id: {}\nfdt address: {:?}", hart_id, fdt_addr);

    let device_tree = unsafe { DeviceTree::new(fdt_addr, &mut serial) };
    let _ = writeln!(&mut serial, "Here!");
    let _ = device_tree.info(&mut serial);

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

    abort();
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
fn panic(_info: &core::panic::PanicInfo) -> ! {
    abort();
}

fn abort() -> ! {
    loop {
        unsafe { riscv::asm::wfi(); }
    }
}