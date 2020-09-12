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

impl <P: Platform> PageFrame<P> {
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

impl <P: Platform> Add<usize> for PageFrame<P> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        PageFrame {
            start: self.start + (rhs * P::MemoryModel::FRAME_SIZE),
            _platform: PhantomData
        }
    }
}