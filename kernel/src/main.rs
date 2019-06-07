#![feature(
    asm,
    stdsimd,
    alloc_error_handler,
    stmt_expr_attributes,
    custom_test_frameworks,
    abi_x86_interrupt,
    impl_trait_in_bindings,
    renamed_spin_loop,
    duration_constants,
    naked_functions,
    global_asm
)]
#![no_std]
#![no_main]
#![reexport_test_harness_main = "test_main"]
#![test_runner(crate::test::test_runner)]

extern crate alloc;

use alloc::vec::Vec;

use bootloader::{bootinfo::BootInfo, entry_point};
use log::info;
use spin::{Mutex, Once};

use serial_logger;

use crate::memory::{frame::FrameAllocator, page_table::PageTableState, KernelAllocator};
use crate::scheduler::context::Context;

mod interrupts;
mod memory;
mod panic;
mod scheduler;
mod system;
mod terminal;
mod time;
mod topology;
mod util;

#[cfg(test)]
mod test;

mod config {
    include!(concat!(env!("OUT_DIR"), "/config.rs"));
}

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

/// Primary initialization of kernel subsystems
pub fn init_core(boot_info: &'static BootInfo) {
    // Order is important
    // 1. Initialize the serial logger so other subsystems can print messages while initializing
    // 2. Initialize the VGA terminal driver in case anything's printed to the screen
    // 3. Initialize the memory system, particularly allocation
    // 4. Initialize the GDT (which allocates an interrupt stack)
    // 5. Initialize the interrupt handlers

    serial_logger::init(&config::MAX_LOG_LEVELS).expect("Could not initialize logging");
    terminal::init();

    let frame_allocator = unsafe { FrameAllocator::initialize(boot_info) };

    memory::bootstrap_allocator(&frame_allocator);

    let page_table_state = PageTableState::initialize(&frame_allocator, boot_info);

    KERNEL_STATE.call_once(|| KernelState {
        frame_allocator,
        page_table_state: Mutex::new(page_table_state),
    });

    system::gdt::init();
    system::pic::init();
    system::apic::init();

    memory::initialize_allocator();
    topology::acpi::discover();
    interrupts::init();
    time::init();

//    crate::system::apic::configure_apic_timer(1);

    system::pic::disable();

    info!("Welcome to Platypos!");
}

pub fn kernel_state<'a>() -> &'a KernelState {
    KERNEL_STATE.wait().expect("Kernel not initialized")
}

#[cfg(not(test))]
fn main(boot_info: &'static BootInfo) -> ! {
    init_core(boot_info);

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

    println!("Time since boot: {:?}", time::current_timestamp());

    let bootstrap_stack_allocation = kernel_state()
        .frame_allocator()
        .allocate_pages(4)
        .expect("Could not allocate bootstrap stack");
    let bootstrap_stack = bootstrap_stack_allocation.start_address() + 4 * 4096u64; // since stack grows down
    let current_pagetable = kernel_state().with_page_table(|pt| pt.current_pml4_location());
    let mut bootstrap_context =
        Context::calling(current_pagetable, bootstrap_stack, bootstrap, 1, 2, 3, 4);

    unsafe { bootstrap_context.make_active() };

    panic!("Bootstrap returned");
}

fn bootstrap(a: usize, b: usize, c: usize, d: usize) -> ! {
    println!("a = {}, b = {}, c = {}, d = {}", a, b, c, d);

    crate::scheduler::init();

    util::hlt_loop();
}

#[alloc_error_handler]
fn handle_alloc_error(layout: ::core::alloc::Layout) -> ! {
    panic!("Could not allocate {} bytes", layout.size());
}

#[cfg(not(test))]
entry_point!(main);
