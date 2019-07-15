use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{PhysFrame, Page};

use hal::{Platform, ProcessorId};

use self::frame_allocator::BuddyBitmapFrameAllocator;

mod frame_allocator;

pub struct X8664Platform {

}

impl Platform for X8664Platform {
    const PAGE_SIZE: usize = 4096;
    type PhysicalAddress = PhysAddr;
    type VirtualAddress = VirtAddr;
    type Page = Page;
    type PageFrame = PhysFrame;
    type FrameAllocator = BuddyBitmapFrameAllocator;

    fn current_processor(&self) -> ProcessorId {
        unimplemented!()
    }

    fn frame_allocator(&self) -> &BuddyBitmapFrameAllocator {
        unimplemented!()
    }
}