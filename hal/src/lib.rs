//! PlatypOS hardware abstraction layer. This abstracts out platform-specific
//! implementations for the kernel and other crates.
#![cfg_attr(not(loom), no_std)]

extern crate alloc;

pub mod interrupts;
pub mod topology;

pub use ciborium_io::{Read, Write};
