mod entry;

pub mod display;
pub mod interrupts;
pub mod mm;

/// Type of the serial port [`::core::fmt::Write`] implementation
pub type SerialPort = uart_16550::SerialPort;

// Address types
pub type PhysicalAddress = x86_64::PhysAddr;
pub type VirtualAddress = x86_64::VirtAddr;

// Paging types. Use VPN / PPN terminology, like the RISC-V spec, rather than
// page and page / physical frame, which gets kind of confusing.
pub type PhysicalPageNumber = x86_64::structures::paging::PhysFrame;
pub type VirtualPageNumber = x86_64::structures::paging::Page;
