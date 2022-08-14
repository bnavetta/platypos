//! Prelude of definitions that most code will need.

pub use core::fmt::Write;

pub use crate::arch::PAGE_SIZE;
pub use crate::error::{Error, ErrorKind};
pub use crate::mm::{
    ByteSizeExt, Page, PageFrame, PageFrameRange, PageRange, PhysicalAddress, PhysicalAddressRange,
    VirtualAddress, VirtualAddressRange,
};

pub use sptr::Strict;

pub use crate::arch::hal_impl;
pub use platypos_hal as hal;

pub type InterruptSafeMutex<'a, T> =
    platypos_common::sync::InterruptSafeMutex<'a, T, hal_impl::interrupts::Controller>;
