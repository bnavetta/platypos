//! x86-64 implementation of the [`Platform`] trait.

use core::convert::Infallible;

use embedded_graphics::pixelcolor::Bgr888;
use platypos_platform::{Platform, Processor};
use x86_64::instructions::interrupts;

use crate::framebuffer::FrameBufferTarget;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct PlatformX86_64 {}

impl Platform for PlatformX86_64 {
    type DisplayColor = Bgr888;
    type DisplayError = Infallible;
    type Display = FrameBufferTarget<'static>;

    type Processor = ProcessorX86_64;

    type Serial = uart_16550::SerialPort;
}

pub struct ProcessorX86_64 {}

pub struct InterruptGuard {
    were_enabled: bool,
}

impl Processor for ProcessorX86_64 {
    type InterruptGuard = InterruptGuard;

    fn disable_interrupts() -> Self::InterruptGuard {
        let were_enabled = interrupts::are_enabled();

        // If interrupts were enabled before this call, disable them while the guard is
        // active
        if were_enabled {
            interrupts::disable();
        }

        InterruptGuard { were_enabled }
    }

    fn halt_until_interrupted() {
        interrupts::enable_and_hlt()
    }
}

impl Drop for InterruptGuard {
    fn drop(&mut self) {
        if self.were_enabled {
            interrupts::enable();
        }
    }
}
