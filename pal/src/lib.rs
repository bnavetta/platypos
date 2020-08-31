//! PlatypOS platform abstraction layer.
#![no_std]
#![feature(const_fn)]

use core::fmt::Write;

pub mod mem;

/// Top-level trait for a platform
pub trait Platform: 'static + Sized {
    type MemoryModel: mem::MemoryModel<Self>;
}
