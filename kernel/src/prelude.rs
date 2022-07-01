//! Prelude of definitions that most code will need.

pub use core::fmt::Write;

pub use crate::arch::PAGE_SIZE;
pub use crate::error::{Error, ErrorKind};
pub use crate::mm::{
    ByteSizeExt, Page, PageFrame, PageFrameRange, PageRange, PhysicalAddress, PhysicalAddressRange,
    VirtualAddress, VirtualAddressRange,
};
