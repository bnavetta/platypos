#![no_std]
#![no_main]
#![feature(asm, abi_efiapi, maybe_uninit_ref, maybe_uninit_slice)]

extern crate alloc;

use alloc::vec;

use log::info;
use platypos_boot_info::BootInfo;
use uefi::prelude::*;
use uefi::table::boot::MemoryType;
use uefi_services;

mod boot_info;
mod elf;
mod file;
mod page_table;
mod util;

use boot_info::BootInfoBuilder;
use file::File;
use page_table::KernelPageTable;
use util::memory_map_size;

/// Memory type for the kernel image
const KERNEL_IMAGE: MemoryType = MemoryType(0x7000_0042);

/// Memory type for loader-allocated kernel data
const KERNEL_DATA: MemoryType = MemoryType(0x7000_0043);

/// Memory type for loader-allocated kernel data that can be reclaimed, such as its initial page table
const KERNEL_RECLAIMABLE: MemoryType = MemoryType(0x7000_0044);

/// Size of one page of memory, defined here for convenience and to avoid magic numbers
const PAGE_SIZE: u64 = 4096;

/// Size of the kernel stack, in pages
const KERNEL_STACK_PAGES: usize = 256;

#[entry]
fn uefi_start(image_handle: uefi::Handle, system_table: SystemTable<Boot>) -> Status {
    uefi_services::init(&system_table).expect_success("Failed to initialize UEFI services");
    log::set_max_level(log::LevelFilter::Trace);

    wait_for_debugger(&system_table, image_handle.clone());

    let kernel_file = File::open(&system_table, "platypos_kernel");
    let mut kernel_object = elf::Object::new(kernel_file);
    let mut page_table = KernelPageTable::new(&system_table);

    let boot_info_builder = BootInfoBuilder::new(&system_table);

    // Allocate the kernel stack right before mapping the kernel executable so that the pages are adjacent, reducing fragmentation.
    // Otherwise, we end up with a mix of perrmanent and reclaimable kernel pages
    let kernel_stack = setup_kernel_stack(&system_table);
    kernel_object.load_and_map(&system_table, &mut page_table);
    page_table.map_loader(&system_table);
    unsafe {
        let boot_info = exit_boot_services(image_handle, system_table, boot_info_builder);
        launch(&kernel_object, &page_table, kernel_stack, boot_info);
    }
}

fn setup_kernel_stack(system_table: &SystemTable<Boot>) -> usize {
    // Allocate 1MiB of stack
    let (stack_phys_addr, stack_data) = util::allocate_frames(system_table, KERNEL_STACK_PAGES, KERNEL_DATA);
    for i in stack_data {
        *i = 0;
    }

    // We don't need to add page table mappings here, because it's handled by KernelPageTable::map_loader

    // The stack grows down
    let stack_top = (stack_phys_addr.as_u64() + KERNEL_STACK_PAGES as u64 * PAGE_SIZE) as usize;
    info!("Kernel stack allocated at {:#x} - {:#x}", stack_phys_addr.as_u64(), stack_top);
    stack_top
}

/// Exits UEFI boot services. This is unsafe because the caller must ensure that no boot services are used after calling this function.
unsafe fn exit_boot_services(
    image_handle: uefi::Handle,
    system_table: SystemTable<Boot>,
    boot_info_builder: BootInfoBuilder,
) -> &'static BootInfo {
    info!("Exiting UEFI boot services");
    let mut map_buf = vec![0u8; memory_map_size(&system_table)];
    let (_, memory_descriptors) = system_table
        .exit_boot_services(image_handle, &mut map_buf)
        .expect_success("Failed to exit UEFI boot services");

    let boot_info = boot_info_builder.generate(memory_descriptors);

    // We can't deallocate the memory map, because it was allocated using UEFI boot services that no longer exist
    ::core::mem::forget(map_buf);

    boot_info
}

unsafe fn launch(kernel: &elf::Object, page_table: &KernelPageTable, kernel_stack: usize, boot_info: &'static BootInfo) -> ! {
    use x86_64::registers::{
        control::{Cr3, Cr3Flags},
        model_specific::{Efer, EferFlags},
    };

    // 1: Enable the no-execute bit, because the kernel page table uses it
    Efer::update(|flags| {
        *flags |= EferFlags::NO_EXECUTE_ENABLE;
    });

    // 2: Switch to the kernel page table
    Cr3::write(page_table.pml4_frame(), Cr3Flags::empty());

    // 3: Jump into the kernel!
    asm!(
        "mov rsp, {kernel_stack}",
        "and rsp, 0xfffffffffffffff0",
        "xor rbp, rbp", // So we start with a null base pointer for backtraces
        "mov rdi, {boot_info}",
        "call {kernel_entry}",
        kernel_stack = in(reg) kernel_stack,
        kernel_entry = in(reg) kernel.metadata.entry().as_u64(),
        boot_info = in(reg) boot_info as *const BootInfo,
        options(noreturn)
    );
}

// TODO: share this code with the kernel

/// The GDB setup script will set this to 1 after it loads symbols.
#[cfg(feature = "gdb")]
static mut DEBUGGER_ATTACHED: u8 = 0;

#[cfg(feature = "gdb")]
fn wait_for_debugger(system_table: &SystemTable<Boot>, image_handle: uefi::Handle) {
    use uefi::proto::loaded_image::LoadedImage;
    let image = system_table
        .boot_services()
        .handle_protocol::<LoadedImage>(image_handle)
        .expect_success("Could not locate loaded image");
    let base_address = unsafe {
        let image = &*image.get();
        let (base_address, _size) = image.info();
        base_address
    };

    info!("Bootloader base address: {:#x}", base_address);
    info!("Waiting for debugger...");
    unsafe {
        while DEBUGGER_ATTACHED == 0 {
            asm!("pause",
              in("r13") base_address,
              options(nomem, nostack, preserves_flags)
            );
        }
    }
}

#[cfg(not(feature = "gdb"))]
fn wait_for_debugger(_system_table: &SystemTable<Boot>, _image_handle: uefi::Handle) {}
