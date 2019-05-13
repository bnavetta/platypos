use log::{warn, trace};
use x86_64::registers::control::Cr2;
use x86_64::structures::idt::{InterruptStackFrame, PageFaultErrorCode};
use x86_64::VirtAddr;

use super::apic::local_apic;
use super::pic;
use crate::interrupts::Interrupt;

pub extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    warn!(
        "Breakpoint exception at {:?}",
        stack_frame.instruction_pointer
    );
}

pub extern "x86-interrupt" fn double_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    _error_code: u64,
) {
    // Error code should always be 0 for a double fault
    panic!(
        "Double fault at {:?} (%rsp = {:?})",
        stack_frame.instruction_pointer, stack_frame.stack_pointer
    );
}

pub extern "x86-interrupt" fn page_fault_handler(
    stack_frame: &mut InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    let addr = Cr2::read();
    panic!(
        "Page fault at {:?}\n    %rsp = {:?}\n    error code = {:?}\n    address = {:?}",
        stack_frame.instruction_pointer, stack_frame.stack_pointer, error_code, addr
    );
}

pub extern "x86-interrupt" fn pic_spurious_interrupt_handler(stack_frame: &mut InterruptStackFrame) {
    warn!("Spurious PIC interrupt!");
//    pic::notify_end_of_interrupt(Interrupt::PicSpurious.as_u8()); // TODO: do these get EOI'd?
}

pub extern "x86-interrupt" fn pic_timer_handler(stack_frame: &mut InterruptStackFrame) {
    trace!("PIC clock interrupt");
    pic::notify_end_of_interrupt(Interrupt::PicTimer.as_u8());
}

pub extern "x86-interrupt" fn apic_timer_handler(stack_frame: &mut InterruptStackFrame) {
    trace!("Clock interrupt!");

    let mut lapic = local_apic();
    lapic.end_of_interrupt();
}

pub extern "x86-interrupt" fn apic_error_handler(stack_frame: &mut InterruptStackFrame) {
    let mut lapic = local_apic();
    panic!("APIC error (ESR = {:#x})", lapic.error_status());
}

pub extern "x86-interrupt" fn apic_spurious_interrupt_handler(stack_frame: &mut InterruptStackFrame) {
    warn!("Spurious interrupt!");
    let mut lapic = local_apic();
    lapic.end_of_interrupt();
}