#![feature(asm)]
#![feature(stdsimd)]
#![feature(alloc_error_handler)]
#![no_std]
#![cfg_attr(not(test), no_main)]

#[macro_use]
extern crate bootloader;
extern crate raw_cpuid;
#[macro_use]
extern crate slog;
extern crate x86_64;

extern crate dbg;
extern crate kutil;

use bootloader::bootinfo::BootInfo;
use dbg::{Category, dbg};
use raw_cpuid::{CpuId, Hypervisor};

use slog_serial::SerialDrain;

pub mod memory;
mod panic;

use memory::alloc::KernelAllocator;

#[global_allocator]
static ALLOC: KernelAllocator = KernelAllocator;

static HELLO: &[u8] = b"Hello World!";

fn main(boot_info: &'static BootInfo) -> ! {
    dbg::init(0x3F8);

    let drain = unsafe { SerialDrain::on_port(0x3F8) };
    let drain = slog::IgnoreResult::new(drain);
     let log = slog::Logger::root_typed(drain, o!("os" => "platypos"));

     info!(log, "This is a log message!");

    let cpuid = CpuId::new();
    match cpuid.get_vendor_info() {
        Some(info) => dbg!(Category::Boot, "CPU: {}", info),
        None => dbg!(Category::Error, "CPUID not supported")
    }

    if let Some(hypervisor) = cpuid.get_hypervisor_info() {
        let hypervisor_name = match hypervisor.identify() {
            Hypervisor::Xen => "Xen",
            Hypervisor::VMware => "VMware",
            Hypervisor::HyperV => "HyperV",
            Hypervisor::KVM => "KVM",
            Hypervisor::Unknown(_, _, _) => "Unknown"
        };
        dbg!(Category::Boot, "Running under {}", hypervisor_name);
    } else {
        dbg!(Category::Boot, "Not running in a hypervisor");
    }

    dbg!(Category::Boot, "Physical Memory Map:");
    for region in boot_info.memory_map.iter() {
        let size = region.range.end_addr() - region.range.start_addr();
        dbg!(Category::Boot, "    {:#018x}-{:#018x}: {:?} ({} bytes)", region.range.start_addr(), region.range.end_addr(), region.region_type, size);
    }

    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {}
}

#[alloc_error_handler]
fn handle_alloc_error(layout: ::core::alloc::Layout) -> ! {
    panic!("Could not allocate {} bytes", layout.size());
}

entry_point!(main);
