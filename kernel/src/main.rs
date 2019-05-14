#![feature(
    asm,
    stdsimd,
    alloc_error_handler,
    stmt_expr_attributes,
    custom_test_frameworks,
    abi_x86_interrupt,
    impl_trait_in_bindings,
    renamed_spin_loop,
    duration_constants
)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![test_runner(crate::test::test_runner)]

extern crate alloc;

use alloc::vec::Vec;

use bootloader::{bootinfo::BootInfo, entry_point};
use log::{debug, info, warn};
use raw_cpuid::{CpuId, Hypervisor};
use spin::{Mutex, Once};

use serial_logger;

use crate::memory::{frame::FrameAllocator, page_table::PageTableState, KernelAllocator};

mod gdt;
mod interrupts;
mod memory;
mod panic;
mod qemu;
mod terminal;
mod timer;
mod util;

#[cfg(test)]
mod test;

#[global_allocator]
static ALLOCATOR: KernelAllocator = KernelAllocator::new();

/// Global container for shared kernel services. This minimizes the number of global Onces floating
/// around and lets init_core enforce subsystem initialization order.
pub struct KernelState {
    frame_allocator: FrameAllocator,
    page_table_state: Mutex<PageTableState>,
}

impl KernelState {
    pub fn frame_allocator(&self) -> &FrameAllocator {
        &self.frame_allocator
    }

    pub fn with_page_table<F, T>(&self, f: F) -> T
    where
        F: FnOnce(&mut PageTableState) -> T,
    {
        let mut state = self.page_table_state.lock();
        f(&mut *state)
    }
}

static KERNEL_STATE: Once<KernelState> = Once::new();

pub fn init_core(boot_info: &'static BootInfo) {
    // Order is important
    // 1. Initialize the serial logger so other subsystems can print messages while initializing
    // 2. Initialize the VGA terminal driver in case anything's printed to the screen
    // 3. Initialize the memory system, particularly allocation
    // 4. Initialize the GDT (which allocates an interrupt stack)
    // 5. Initialize the interrupt handlers

    serial_logger::init().expect("Could not initialize logging");
    terminal::init();

    let frame_allocator = unsafe { FrameAllocator::initialize(boot_info) };

    memory::bootstrap_allocator(&frame_allocator);

    KERNEL_STATE.call_once(|| KernelState {
        frame_allocator,
        page_table_state: Mutex::new(PageTableState::initialize(boot_info)),
    });

    gdt::init();
    timer::pit::init();
    interrupts::init();
    memory::initialize_allocator();

    info!("Welcome to Platypos!");
}

pub fn kernel_state<'a>() -> &'a KernelState {
    KERNEL_STATE.wait().expect("Kernel not initialized")
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

    let mut v = Vec::new();
    for i in 0..10 {
        v.push(i);
    }
    println!("v = {:?}", v);

    util::hlt_loop();
}

#[alloc_error_handler]
fn handle_alloc_error(layout: ::core::alloc::Layout) -> ! {
    panic!("Could not allocate {} bytes", layout.size());
}

#[cfg(not(test))]
entry_point!(main);
