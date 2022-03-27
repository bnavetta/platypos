//! Platform-agnostic representation of physical memory frames

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::Platform;
use super::MemoryModel;
use super::address::PhysicalAddress;

#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PageFrame<P: Platform> {
    start: PhysicalAddress<P>,
    _platform: PhantomData<&'static P>
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct PageFrameRange<P: Platform> {
    start: PageFrame<P>,
    frames: usize
}

impl <P: Platform> PageFrame<P> {
    /// Create a new `PageFrame` from its starting address.
    ///
    /// # Panics
    /// If `start` is not page-aligned
    pub fn from_start(start: PhysicalAddress<P>) -> PageFrame<P> {
        // TODO: is this the only requirement across all platforms? May need address-like validation
        assert_eq!(start.into_inner() % P::MemoryModel::FRAME_SIZE, 0, "Start address {} is not page-aligned", start);
        PageFrame { start, _platform: PhantomData }
    }

    pub fn start(self) -> PhysicalAddress<P> {
        self.start
    }

    /// The "frame number" of this frame, which is defined as its starting address divided by the
    /// frame size. This usually isn't significant to the hardware, but is helpful when indexing
    /// on page frames.
    pub const fn frame_number(self) -> usize {
        self.start.into_inner() / P::MemoryModel::FRAME_SIZE
    }
}

impl <P: Platform> fmt::Debug for PageFrame<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PageFrame({:#x})", self.start)
    }
}

impl <P: Platform> fmt::Display for PageFrame<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.start)
    }
}

impl <P: Platform> Add<usize> for PageFrame<P> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        PageFrame {
            start: self.start + (rhs * P::MemoryModel::FRAME_SIZE),
            _platform: PhantomData
        }
    }
}

impl <P: Platform> Sub<usize> for PageFrame<P> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        PageFrame {
            start: self.start - (rhs * P::MemoryModel::FRAME_SIZE),
            _platform: PhantomData
        }
    }
}

impl <P: Platform> PageFrameRange<P> {
    pub fn new(start: PageFrame<P>, frames: usize) -> PageFrameRange<P> {
        PageFrameRange { start, frames }
    }

    /// The starting frame of this range
    pub fn start(&self) -> PageFrame<P> {
        self.start
    }

    /// The exclusive ending frame of this range. A `PageFrameRange` includes all page frames
    /// such that `f >= start` and `f < end`.
    pub fn end(&self) -> PageFrame<P> {
        self.start + self.frames
    }

    /// The length of this range, in page frames
    pub fn frames(&self) -> usize {
        self.frames
    }

    /// The length of this range, in bytes
    pub fn bytes(&self) -> usize {
        self.frames * P::MemoryModel::FRAME_SIZE
    }
}

impl <P: Platform> fmt::Display for PageFrameRange<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} - {}", self.start(), self.end())
    }
}