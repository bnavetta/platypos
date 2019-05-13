//! Interrupt handler configuration. This module is responsible for setting up PlatypOS' IDT and
//! providing interrupt handlers.
use log::info;
use spin::Once;
use x86_64::{
    instructions::interrupts as int,
    structures::idt::InterruptDescriptorTable,
};

static IDT: Once<InterruptDescriptorTable> = Once::new();

mod apic;
mod handlers;
mod pic;

#[repr(u8)]
#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum Interrupt {
    PicTimer = 32,     // IRQ 0
    PicSpurious = 39,  // IRQ 7 for PIC 1 spurious interrupts
    PicSpurious2 = 47, // IRQ 15 for PIC 2 spurious interrupts
    ApicTimer = 48,
    ApicError = 49,
    ApicSpurious = 255,
}

impl Interrupt {
    pub fn as_u8(self) -> u8 {
        self as u8
    }

    pub fn as_u32(self) -> u32 {
        self.as_u8() as u32
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

        // Maybe reusing the handlers isn't the best idea?
        idt[Interrupt::PicSpurious.as_usize()]
            .set_handler_fn(self::handlers::apic_spurious_interrupt_handler);
        idt[Interrupt::PicSpurious2.as_usize()]
            .set_handler_fn(self::handlers::apic_spurious_interrupt_handler);
        idt[Interrupt::ApicSpurious.as_usize()]
            .set_handler_fn(self::handlers::apic_spurious_interrupt_handler);

        idt[Interrupt::ApicTimer.as_usize()].set_handler_fn(self::handlers::apic_timer_handler);
        idt[Interrupt::ApicError.as_usize()].set_handler_fn(self::handlers::apic_error_handler);

        idt[Interrupt::PicTimer.as_usize()].set_handler_fn(self::handlers::pic_timer_handler);

        idt
    });

    idt.load();

    // There are a couple order-dependent steps:
    // 1. Configure the PIC and LAPIC
    // 2. Enable interrupts (so we get interrupts from the PIT)
    // 3. Configure the LAPIC timer using the PIT
    // 4. Disable the PIC now that we don't need it anymore

    unsafe {
        pic::initialize_pic();
    }

    apic::configure_local_apic();

    info!("Enabling interrupts");
    int::enable();

    apic::configure_apic_timer(crate::timer::apic::TIMER_FREQUENCY_HZ as u32);
    pic::disable_pic();
    crate::timer::set_source(crate::timer::TimerSource::LocalApicTimer);
}

#[cfg(test)]
mod tests {
    use crate::tests;

    tests! {
        test breakpoint_exception {

        }
    }
}
