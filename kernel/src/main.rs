#![feature(asm, stdsimd, alloc_error_handler, stmt_expr_attributes, custom_test_frameworks)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![test_runner(crate::test::test_runner)]

#[macro_use]
extern crate bootloader;
extern crate raw_cpuid;
extern crate x86_64;

extern crate kutil;

use bootloader::bootinfo::BootInfo;
use log::{debug, info, warn};
use raw_cpuid::{CpuId, Hypervisor};
use serial_logger;

mod memory;
mod panic;
mod qemu;

#[cfg(test)]
mod test;

use memory::alloc::KernelAllocator;

#[global_allocator]
static ALLOC: KernelAllocator = KernelAllocator;

static HELLO: &[u8] = b"Hello World!";

#[cfg(not(test))]
fn main(boot_info: &'static BootInfo) -> ! {
    serial_logger::init().expect("Could not initialize logging");

    let cpuid = CpuId::new();
    match cpuid.get_vendor_info() {
        Some(info) => debug!("CPU: {}", info),
        None => warn!("CPUID not supported"),
    }

    if let Some(hypervisor) = cpuid.get_hypervisor_info() {
        let hypervisor_name = match hypervisor.identify() {
            Hypervisor::Xen => "Xen",
            Hypervisor::VMware => "VMware",
            Hypervisor::HyperV => "HyperV",
            Hypervisor::KVM => "KVM",
            Hypervisor::Unknown(_, _, _) => "Unknown",
        };
        debug!("Running under {}", hypervisor_name);
    } else {
        debug!("Not running in a hypervisor");
    }

    info!("Physical Memory Map:");
    for region in boot_info.memory_map.iter() {
        let size = region.range.end_addr() - region.range.start_addr();
        info!(
            "    {:#018x}-{:#018x}: {:?} ({} bytes)",
            region.range.start_addr(),
            region.range.end_addr(),
            region.region_type,
            size
        );
    }

    memory::frame::init(boot_info);

    let mut blocks = [None; 50];

    for i in 0..blocks.len() {
        blocks[i] = memory::frame::allocate_frames(16);
    }

    blocks.iter().flatten().for_each(|block| memory::frame::free_frames(*block, 16));

    let vga_buffer = 0xb8000 as *mut u8;

    for (i, &byte) in HELLO.iter().enumerate() {
        unsafe {
            *vga_buffer.offset(i as isize * 2) = byte;
            *vga_buffer.offset(i as isize * 2 + 1) = 0xb;
        }
    }

    loop {
        x86_64::instructions::hlt();
    }
}

#[alloc_error_handler]
fn handle_alloc_error(layout: ::core::alloc::Layout) -> ! {
    panic!("Could not allocate {} bytes", layout.size());
}

#[cfg(not(test))]
entry_point!(main);
