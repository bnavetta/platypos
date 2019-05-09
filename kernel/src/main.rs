#![feature(
    asm,
    stdsimd,
    alloc_error_handler,
    stmt_expr_attributes,
    custom_test_frameworks,
    abi_x86_interrupt,
    impl_trait_in_bindings
)]
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
use x86_64::VirtAddr;

mod gdt;
mod interrupts;
mod memory;
mod panic;
mod qemu;
mod terminal;
mod util;

#[cfg(test)]
mod test;

pub fn init_core(boot_info: &'static BootInfo) {
    // Order is important
    // 1. Initialize the serial logger so other subsystems can print messages while initializing
    // 2. Initialize the VGA terminal driver in case anything's printed to the screen
    // 3. Initialize the memory system, particularly allocation
    // 4. Initialize the GDT (which allocates an interrupt stack)
    // 5. Initialize the interrupt handlers

    serial_logger::init().expect("Could not initialize logging");
    terminal::init();
    memory::init(boot_info);
    gdt::init();
    interrupts::init();

    info!("Welcome to Platypos!");
}

#[cfg(not(test))]
fn main(boot_info: &'static BootInfo) -> ! {
    init_core(boot_info);

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

    x86_64::instructions::interrupts::int3();

    self::memory::page_table::with_page_table(|pt| {
        let addresses = [
            // the identity-mapped vga buffer page
            0xb8000,
            // some code page
            0x20010a,
            // some stack page
            0x57ac_001f_fe48,
            // virtual address mapped to physical address 0
            boot_info.physical_memory_offset,
        ];

        for &address in &addresses {
            let addr = VirtAddr::new(address);
            println!("{:?} is mapped to {:?}", addr, pt.translate(addr));
        }
    });

    //    let mut blocks = [None; 50];
    //
    //    for i in 0..blocks.len() {
    //        blocks[i] = memory::frame::allocate_frames(16);
    //    }
    //
    //    blocks.iter().flatten().for_each(|block| memory::frame::free_frames(*block, 16));

    let mut allocator = crate::memory::alloc::KernelAllocator::new();
    assert_eq!(allocator.allocate(42), None);

    println!("Welcome to PlatypOS! :)");

    unsafe {
        *(0xdeadbeef as *mut u64) = 42;
    };

    util::hlt_loop();
}

#[alloc_error_handler]
fn handle_alloc_error(layout: ::core::alloc::Layout) -> ! {
    panic!("Could not allocate {} bytes", layout.size());
}

#[cfg(not(test))]
entry_point!(main);
