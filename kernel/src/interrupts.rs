//! Interrupt handler configuration. This module is responsible for setting up PlatypOS' IDT and
//! providing interrupt handlers.
use log::info;
use spin::Once;
use x86_64::structures::idt::InterruptDescriptorTable;
use x86_64::instructions::interrupts as int;
use x86_64::instructions::port::Port;

static IDT: Once<InterruptDescriptorTable> = Once::new();

mod apic;
mod handlers;

const INTERRUPT_TIMER: u8 = 32;
const INTERRUPT_SPURIOUS: u8 = 39;
const INTERRUPT_APIC_ERROR: u8 = 255;

pub fn init() {
    assert!(!int::are_enabled(), "Interrupts unexpectedly enabled");

    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint
            .set_handler_fn(self::handlers::breakpoint_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(self::handlers::double_fault_handler)
                .set_stack_index(crate::gdt::FAULT_IST_INDEX);
        }
        unsafe {
            idt.page_fault
                .set_handler_fn(self::handlers::page_fault_handler)
                .set_stack_index(crate::gdt::FAULT_IST_INDEX);
        }

        idt[INTERRUPT_TIMER as usize].set_handler_fn(self::handlers::clock_interrupt_handler);

        idt[INTERRUPT_SPURIOUS as usize].set_handler_fn(self::handlers::spurious_interrupt_handler);
        idt[INTERRUPT_APIC_ERROR as usize].set_handler_fn(self::handlers::apic_error_interrupt_handler);

        idt
    });

    idt.load();

    disable_pic();
    apic::configure_local_apic();

    info!("Enabling interrupts");
    int::enable();
}

/// Disable the 8259 PIC
fn disable_pic() {
    // https://wiki.osdev.org/8259_PIC#Disabling

    let mut pic1: Port<u8> = Port::new(0xA1);
    unsafe { pic1.write(0xff); }

    let mut pic2: Port<u8> = Port::new(0x21);
    unsafe { pic2.write(0xff); }
}

#[cfg(test)]
mod tests {
    use crate::tests;

    tests! {
        test breakpoint_exception {

        }
    }
}
