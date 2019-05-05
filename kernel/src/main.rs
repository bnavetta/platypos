#![feature(asm)]
#![feature(stdsimd)]
#![no_std]
#![cfg_attr(not(test), no_main)]

#[macro_use]
extern crate bootloader_precompiled;
extern crate raw_cpuid;
extern crate x86_64;

extern crate dbg;
extern crate kutil;

use bootloader_precompiled::bootinfo::BootInfo;
use dbg::{Category, dbg};
use raw_cpuid::{CpuId, Hypervisor};

pub mod memory;
mod panic;

static HELLO: &[u8] = b"Hello World!";

fn main(boot_info: &'static BootInfo) -> ! {
    dbg::init(0x3F8);

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

entry_point!(main);
