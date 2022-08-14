use platypos_common::sync::Global;
use x86_64::instructions::interrupts;
use x86_64::structures::idt::InterruptDescriptorTable;

use platypos_hal as hal;

mod apic;
mod handlers;

#[derive(Debug, Clone, Copy)]
pub struct Controller;

static GLOBAL: Global<Controller> = Global::new();

/// Interrupt descriptor table. For now, use the same one on all processors.
static IDT: Global<InterruptDescriptorTable> = Global::new();

/// IRQ that spurious interrupts are mapped to (see Intel SDM vol 3A, 10.9)
/// See the OSDev wiki for more information, but 0xff is an easy default for
/// this:
/// * It's above 32, and so not reserved for exceptions
/// * Its lowest 4 bits are set, which some hardware requires
const SPURIOUS_INTERRUPT_VECTOR: u8 = 0xff;

/// Configure the interrupt controller
pub fn init() -> &'static Controller {
    apic::disable_pic();

    // TODO: will this force an expensive move?
    let mut idt = InterruptDescriptorTable::new();
    for off in 0..8 {
        idt[(apic::PIC1_OFFSET + off).into()].set_handler_fn(handlers::handle_remapped_pic);
        idt[(apic::PIC2_OFFSET + off).into()].set_handler_fn(handlers::handle_remapped_pic);
    }
    idt[SPURIOUS_INTERRUPT_VECTOR.into()].set_handler_fn(handlers::handle_spurious);
    IDT.init(idt);

    GLOBAL.init(Controller)
}

/// Perform processor-local initialization
pub fn init_local() {
    apic::init_local();
    IDT.get().load();
}

impl hal::interrupts::Controller for Controller {
    fn force_enable(&self) {
        interrupts::enable()
    }

    fn force_disable(&self) {
        interrupts::disable()
    }

    fn enabled(&self) -> bool {
        interrupts::are_enabled()
    }

    fn wait(&self) {
        interrupts::enable_and_hlt()
    }
}
