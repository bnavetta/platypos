use bit_field::BitField;
use log::debug;
use pic8259_simple::ChainedPics;
use spin::{Mutex, Once};
use x86_64::instructions::port::Port;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub struct ProgrammableInterruptController {
    pics: ChainedPics,
    pic1_data: Port<u8>,
    pic2_data: Port<u8>,
}

impl ProgrammableInterruptController {
    unsafe fn initialize() -> ProgrammableInterruptController {
        let mut pic = ProgrammableInterruptController {
            pics: ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET),
            pic1_data: Port::new(0x21),
            pic2_data: Port::new(0xa1),
        };

        pic.pics.initialize();
        pic
    }

    fn notify_end_of_interrupt(&mut self, interrupt: u8) {
        unsafe {
            self.pics.notify_end_of_interrupt(interrupt);
        }
    }

    /// Disable the 8259 PIC.
    ///
    /// See https://wiki.osdev.org/8259_PIC#Disabling
    fn disable(&mut self) {
        unsafe {
            self.pic2_data.write(0xff);
            self.pic1_data.write(0xff);
        }
    }

    unsafe fn set_masked(&mut self, irq: u8, masked: bool) {
        let port = if irq < 8 {
            &mut self.pic1_data
        } else {
            &mut self.pic2_data
        };

        let mut mask = port.read();
        mask.set_bit(irq as usize, masked);
        port.write(mask);
    }

    fn log_masks(&mut self) {
        let pic1_mask = unsafe { self.pic1_data.read() };
        debug!("PIC1 mask = {:b}", pic1_mask);
        let pic2_mask = unsafe { self.pic2_data.read() };
        debug!("PIC2 mask = {:b}", pic2_mask);
    }
}

// unfortunately has to be global so interrupt handlers can signal EOI.
static PIC: Once<Mutex<ProgrammableInterruptController>> = Once::new();

pub fn notify_end_of_interrupt(interrupt: u8) {
    let mut pic = PIC.wait().expect("PIC not initialized").lock();
    pic.notify_end_of_interrupt(interrupt);
}

/// Initialize the chained 8259 PICs, remapping their IRQs.
pub fn init() {
    PIC.call_once(|| {
        Mutex::new(unsafe {
            let mut pic = ProgrammableInterruptController::initialize();
            // ensure PIT IRQs are unmasked so we can use them to calibrate other timers
            pic.disable();
            pic.set_masked(2, false); // IRQ used for chaining
            pic.set_masked(0, false);
            pic.log_masks();
            pic
        })
    });
}

/// Disable the 8259 PIC
pub fn disable() {
    let mut pic = PIC.wait().expect("PIC not initialized").lock();
    pic.disable();
}
