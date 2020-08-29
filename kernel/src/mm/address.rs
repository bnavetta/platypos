//! Platform-agnostic address types

use core::fmt;
use core::ops::Sub;

use crate::arch::mm;

/// A virtual memory address. Note that this does not carry any information about _which_ address spaces this is valid in.
pub struct VirtualAddress(usize);

/// A physical memory address.
pub struct PhysicalAddress(usize);

/// A page-sized frame of physical memory
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PageFrame(usize);

impl PageFrame {
    /// Creates a new `PageFrame` representing the given page frame number. Page frame numbers have the same meaning as in Linux.
    #[inline]
    pub const fn new(frame_number: usize) -> PageFrame {
        PageFrame(frame_number)
    }

    #[inline]
    pub const fn frame_number(self) -> usize {
        self.0
    }

    #[inline]
    pub const fn start_address(self) -> PhysicalAddress {
        PhysicalAddress(self.0 * mm::PAGE_SIZE)
    }
}

impl fmt::Debug for PageFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PageFrame({:#x})", self.0)
    }
}

/// The number of page frames between this and the other PageFrame
impl Sub for PageFrame {
    type Output = usize;

    fn sub(self, other: PageFrame) -> usize {
        self.0 - other.0
    }
}

impl slog::Value for PageFrame {
    fn serialize(&self, record: &slog::Record, key: slog::Key, serializer: &mut dyn slog::Serializer) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{:?}", self))
    }
}