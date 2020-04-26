use x86_64::structures::paging::{PageSize, Page};
use x86_64::structures::paging::page::PageRange;
use x86_64::structures::paging::frame::{PhysFrame, PhysFrameRange};

pub trait PageSizeExt {
    /// Returns the number of pages needed to contain `size` bytes
    fn pages_containing(size: usize) -> usize;
}

impl <S: PageSize> PageSizeExt for S {
    #[inline]
    fn pages_containing(size: usize) -> usize {
        ((size as u64 + S::SIZE - 1) / S::SIZE) as usize
    }
}

pub trait PageExt<S: PageSize> {
    /// Returns a page range starting at this page and including `pages` pages
    fn range_to(self, pages: usize) -> PageRange<S>;
}

impl <S: PageSize> PageExt<S> for Page<S> {
    #[inline]
    fn range_to(self, pages: usize) -> PageRange<S> {
        Page::range(self, self + pages as u64)
    }
}

pub trait PhysFrameExt<S: PageSize> {
    /// Returns a physical frame range starting at this frame and including `frames` frames
    fn range_to(self, frames: usize) -> PhysFrameRange<S>;
}

impl <S: PageSize> PhysFrameExt<S> for PhysFrame<S> {
    #[inline]
    fn range_to(self, frames: usize) -> PhysFrameRange<S> {
        PhysFrame::range(self, self + frames as u64)
    }
}