use core::fmt::Write;
use core::hint::unreachable_unchecked;

use uefi::table::Runtime;
use x86_64::{VirtAddr, PhysAddr};
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::registers::model_specific::{Efer, EferFlags};
use x86_64::structures::paging::PhysFrame;
use uart_16550::SerialPort;

use super::{BootManager, Stage, KERNEL_STACK_HIGH};

pub struct Handoff {
    /// Address of the kernel entry point
    pub kernel_entry_addr: VirtAddr,
    pub debug_port: SerialPort,
}

impl Stage for Handoff {
    type SystemTableView = Runtime;
}

impl BootManager<Handoff> {
    /// Hand off control to the kernel, jumping to its start address
    pub fn handoff(mut self) -> ! {
        // At this point, we're post-boot-services, so logging won't work. Instead, a reference to
        // the debug serial port is passed along from the previous stage.

        self.enable_no_execute();

        unsafe {
            self.activate_page_table(self.page_table_address);
            self.switch_to_kernel();
        }
    }

    /// Returns a reference to the serial port used for debug output
    pub fn debug_port(&mut self) -> &mut SerialPort {
        // This is exposed so that BootManager users can also log things if needed
        &mut self.stage.debug_port
    }

    /// Sets the no-execute enable flag in the EFER MSR, which allows using the NO_EXECUTE
    /// bit in page tables
    fn enable_no_execute(&mut self) {
        writeln!(&mut self.stage.debug_port, "Enabling no-execute bit in EFER");
        unsafe {
            Efer::update(|efer| *efer |= EferFlags::NO_EXECUTE_ENABLE);
        }
    }

    /// Switch to the given page table.
    unsafe fn activate_page_table(&mut self, pml4_addr: PhysAddr) {
        writeln!(&mut self.stage.debug_port, "Switching to page table at {:#x}", pml4_addr);
        let frame = PhysFrame::from_start_address(pml4_addr).expect("PML4 address is not page-aligned");
        Cr3::write(frame, Cr3Flags::empty());
    }

    unsafe fn switch_to_kernel(&mut self) -> ! {
        writeln!(&mut self.stage.debug_port, "Jumping into kernel at {:#x}", self.stage.kernel_entry_addr.as_u64());
        asm!("pushq $0\n\t\
              retq\n\t\
              hlt" : : "r"(self.stage.kernel_entry_addr), "{rsp}"(KERNEL_STACK_HIGH) : "memory" : "volatile");
        unreachable_unchecked();
    }
}