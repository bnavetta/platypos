//! PlatypOS hardware abstraction layer. This abstracts out platform-specific
//! implementations for the kernel and other crates.
#![no_std]

pub mod interrupts;

pub use ciborium_io::{Read, Write};
