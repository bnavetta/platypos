//! PlatypOS platform abstraction layer.
#![no_std]
#![feature(const_fn)]

#[macro_use]
extern crate bitflags;

use core::any::Any;
use core::fmt::Debug;

pub mod mem;

/// Top-level trait for a platform
pub trait Platform: 'static + Sized + Eq + Copy + Clone + Debug + Any + Send + Sync {
    type MemoryModel: mem::MemoryModel<Self>;
}

// TODO: platform abstraction over interrupt-aware spinlocks that automatically mask/unmask interrupts
//       at a specific level while lock is held (but no guarantees about which levels cover which other
//       levels)