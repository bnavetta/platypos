//! Interrupt handler configuration. This module is responsible for setting up PlatypOS' IDT and
//! providing interrupt handlers.
use log::warn;
use spin::Once;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame};

static IDT: Once<InterruptDescriptorTable> = Once::new();

pub fn init() {
    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt
    });

    idt.load();
}

extern "x86-interrupt" fn breakpoint_handler(stack_frame: &mut InterruptStackFrame) {
    warn!("Breakpoint exception at {:?}", stack_frame.instruction_pointer);
}

#[cfg(test)]
mod tests {
    use crate::tests;

    tests! {
        test breakpoint_exception {

        }
    }
}