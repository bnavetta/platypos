use core::fmt::Write;
use core::hint::unreachable_unchecked;
use core::mem;
use alloc::vec;

use log::info;

use uart_16550::SerialPort;

use uefi::prelude::*;

use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging::{PageSize, Size4KiB};

use crate::kernel_image::KernelImage;
use crate::page_table::KernelPageTable;
use crate::memory_map::{KERNEL_STACK_START, KERNEL_STACK_PAGES};

/// Hands off to the kernel by exiting UEFI boot services and jumping to its entry point.
pub fn handoff(loaded_image: Handle, system_table: SystemTable<Boot>, kernel_image: &KernelImage, page_table: &KernelPageTable) -> ! {
    let mut debug_port = unsafe { SerialPort::new(0x3F8) };

    exit_boot_services(&mut debug_port, loaded_image, system_table);
    unsafe {
        activate_page_table(&mut debug_port, page_table);
        jump_to_kernel(&mut debug_port, kernel_image);
    }
}

/// Exits UEFI boot services
fn exit_boot_services(debug_port: &mut SerialPort, loaded_image: Handle, system_table: SystemTable<Boot>) {
    // Add padding in case the memory map grows between now and calling exit_boot_services
    let mut memory_map_buf = vec![0u8; system_table.boot_services().memory_map_size() + 256];

    info!("Exiting UEFI boot services");

    debug_port.init();

    let (system_table, memory_map) = match system_table.exit_boot_services(loaded_image, &mut memory_map_buf) {
        Ok(completion) => {
            let (status, res) = completion.split();
            if status.is_success() {
                res
            } else {
                let _ = writeln!(debug_port, "Warning exiting UEFI boot services: {:?}", status);
                halt_loop();
            }
        },
        Err(err) => {
            let _ = writeln!(debug_port, "Error exiting UEFI boot services: {:?}", err);
            halt_loop();
        }
    };

    let _ = writeln!(debug_port, "Exited UEFI boot services");

    // mem::forget to prevent calling the UEFI allocator after exiting boot services
    mem::forget(memory_map_buf);
}

/// Switches to the transitional kernel page table. The transitional page table must contain both mappings needed for the loader
/// and mappings needed for the kernel
unsafe fn activate_page_table(debug_port: &mut SerialPort, page_table: &KernelPageTable) {
    // Set the no-execute enable flag. Otherwise, switching to a page table using NO_EXECUTE bits will fail
    Efer::update(|efer| *efer |= EferFlags::NO_EXECUTE_ENABLE);

    let _ = writeln!(debug_port, "Activating kernel page table at {:?}", page_table.page_table_frame());
    Cr3::write(page_table.page_table_frame(), Cr3Flags::empty());
}

/// Jumps to the kernel's entry point
unsafe fn jump_to_kernel(debug_port: &mut SerialPort, kernel: &KernelImage) -> ! {
    let kernel_stack_pointer = KERNEL_STACK_START + KERNEL_STACK_PAGES * Size4KiB::SIZE as usize;

    let _ = writeln!(debug_port, "Jumping into kernel");
    llvm_asm!("pushq $0\n\t\
          retq\n\t\
          hlt" : : "r"(kernel.entry_address().as_u64()), "{rsp}"(kernel_stack_pointer) : "memory" : "volatile");
    unreachable_unchecked()
}

/// Calls `hlt` in a loop, for if things go wrong
fn halt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}