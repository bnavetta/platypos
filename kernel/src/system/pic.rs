use pic8259_simple::ChainedPics;
use spin::{Mutex, Once};
use x86_64::instructions::port::Port;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

// unfortunately has to be global so interrupt handlers can signal EOI.
static PICS: Once<Mutex<ChainedPics>> = Once::new();

pub fn notify_end_of_interrupt(interrupt: u8) {
    let mut pics = PICS.wait().expect("PICs not initialized").lock();
    unsafe {
        pics.notify_end_of_interrupt(interrupt);
    }
}

/// Initialize the chained 8259 PICs, remapping their IRQs.
pub fn init() {
    PICS.call_once(|| {
        Mutex::new(unsafe {
            let mut pics = ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET);
            pics.initialize();
            pics
        })
    });
}

/// Disable the 8259 PIC
pub fn disable() {
    // https://wiki.osdev.org/8259_PIC#Disabling

    let mut pic1: Port<u8> = Port::new(0xA1);
    unsafe {
        pic1.write(0xff);
    }

    let mut pic2: Port<u8> = Port::new(0x21);
    unsafe {
        pic2.write(0xff);
    }
}
