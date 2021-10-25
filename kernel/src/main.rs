#![no_std]
#![no_main]
#![feature(panic_info_message, asm, global_asm, array_chunks)]

use core::fmt::Write;

use driver::uart::Uart;
use fdt_rs::base::iters::StringPropIter;
use fdt_rs::index::DevTreeIndexNode;
use fdt_rs::prelude::*;
use fdt_rs::base::*;

// #[cfg_attr(target_arch="riscv", path="arch/riscv.rs")]
#[path ="arch/riscv.rs"]
mod arch;

mod driver;

#[no_mangle]
extern "C" fn kmain(hart_id: usize, fdt_addr: *const u8) {
    let mut driver = unsafe {
        let mut uart = crate::driver::uart::Uart::new(0x1000_0000);
        uart.init();
        uart
    };

    let _ = writeln!(&mut driver, "Hello, World!");
    let _ = writeln!(&mut driver, "hart id: {}\nfdt address: {:?}", hart_id, fdt_addr);

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
}

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

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    abort();
}

fn abort() -> ! {
    loop {
        unsafe { riscv::asm::wfi(); }
    }
}