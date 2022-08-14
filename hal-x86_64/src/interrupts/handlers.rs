//! Interrupt handler entry points

use x86_64::structures::idt::InterruptStackFrame;

pub extern "x86-interrupt" fn handle_remapped_pic(_frame: InterruptStackFrame) {
    tracing::warn!("Got an interrupt from the PIC");
}

pub extern "x86-interrupt" fn handle_spurious(_frame: InterruptStackFrame) {
    tracing::warn!("Got a spurious interrupt");
}
