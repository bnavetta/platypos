mod entry;

pub mod display;
pub mod interrupts;
pub mod mm;

/// Type of the serial port [`::core::fmt::Write`] implementation
pub type SerialPort = uart_16550::SerialPort;

// Paging types. Use VPN / PPN terminology, like the RISC-V spec, rather than
// page and page / physical frame, which gets kind of confusing.
pub type PhysicalPageNumber = x86_64::structures::paging::PhysFrame;
pub type VirtualPageNumber = x86_64::structures::paging::Page;

pub const PAGE_SIZE: usize = 4096;
