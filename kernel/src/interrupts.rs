//! Interrupt handler configuration. This module is responsible for setting up PlatypOS' IDT and
//! providing interrupt handlers.
use log::info;
use spin::Once;
use x86_64::{instructions::interrupts as int, structures::idt::InterruptDescriptorTable, VirtAddr};

use crate::system::gdt::FAULT_IST_INDEX;
use crate::system::pic::{PIC_1_OFFSET, PIC_2_OFFSET};

static IDT: Once<InterruptDescriptorTable> = Once::new();

mod handlers;

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Interrupt {
    PicTimer = PIC_1_OFFSET,         // IRQ 0
    PicSpurious = PIC_1_OFFSET + 7,  // IRQ 7 for PIC 1 spurious interrupts
    PicSpurious2 = PIC_2_OFFSET + 7, // IRQ 15 for PIC 2 spurious interrupts
    ApicTimer = 48,
    ApicSpurious = 255,
}

impl Interrupt {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_usize(self) -> usize {
        self.as_u8() as usize
    }
}

pub fn init() {
    assert!(!int::are_enabled(), "Interrupts unexpectedly enabled");

    let idt = IDT.call_once(|| {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint
            .set_handler_fn(self::handlers::breakpoint_handler);
        idt.general_protection_fault
            .set_handler_fn(self::handlers::general_protection_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(self::handlers::double_fault_handler)
                .set_stack_index(FAULT_IST_INDEX);
        }
        unsafe {
            idt.page_fault
                .set_handler_fn(self::handlers::page_fault_handler)
                .set_stack_index(FAULT_IST_INDEX);
        }

        idt[Interrupt::PicSpurious.as_usize()]
            .set_handler_fn(self::handlers::pic_spurious_interrupt_handler);
        idt[Interrupt::PicSpurious2.as_usize()]
            .set_handler_fn(self::handlers::pic2_spurious_interrupt_handler);
        idt[Interrupt::ApicSpurious.as_usize()]
            .set_handler_fn(self::handlers::apic_spurious_interrupt_handler);

        idt[Interrupt::ApicTimer.as_usize()].set_handler_fn(self::handlers::apic_timer_handler);

        idt[Interrupt::PicTimer.as_usize()].set_handler_fn(self::handlers::pic_timer_handler);

        idt
    });

    idt.load();

    info!("Enabling interrupts");
    int::enable();
}

/// Install the IDT on the current processor, and enable interrupts. This only needs to be called on
/// application processors, as the bootstrap processor installs the IDT after creating it.
pub fn install() {
    // TODO: per-processor IDTs so we have more IRQs to work with?

    IDT.wait().expect("IDT not created").load();
    int::enable();
}

#[cfg(test)]
mod tests {
    use crate::tests;

    tests! {
        test breakpoint_exception {

        }
    }
}
