//! Interrupt handler configuration. This module is responsible for setting up PlatypOS' IDT and
//! providing interrupt handlers.
use log::warn;
use spin::Once;
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};

static IDT: Once<InterruptDescriptorTable> = Once::new();

pub fn init() {
    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(crate::gdt::FAULT_IST_INDEX);
        }
        unsafe {
            idt.page_fault
                .set_handler_fn(page_fault_handler)
                .set_stack_index(crate::gdt::FAULT_IST_INDEX);
        }
        idt
    });

    idt.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    warn!(
        "Breakpoint exception at {:?}",
        stack_frame.instruction_pointer
    );
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) {
    // Error code should always be 0 for a double fault
    panic!(
        "Double fault at {:?} (%rsp = {:?})",
        stack_frame.instruction_pointer, stack_frame.stack_pointer
    );
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let addr = Cr2::read();
    panic!(
        "Page fault at {:?}\n    %rsp = {:?}\n    error code = {:?}\n    address = {:?}",
        stack_frame.instruction_pointer, stack_frame.stack_pointer, error_code, addr
    );
}

#[cfg(test)]
mod tests {
    use crate::tests;

    tests! {
        test breakpoint_exception {

        }
    }
}
