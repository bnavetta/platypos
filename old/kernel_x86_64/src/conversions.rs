//! Conversions between PAL types and types from the x86-64 crate.

use platypos_pal::mem::{PhysicalAddress, PageFrame};

use crate::platform::Platform;

/// Conversion trait for going from an x86-64 type to a PAL type
pub trait IntoPal<T> {
    fn into_pal(self) -> T;
}

impl IntoPal<PhysicalAddress<Platform>> for x86_64::PhysAddr {
    fn into_pal(self) -> PhysicalAddress<Platform> {
        // Safety: the x86-64 crate also validates addresses
        unsafe { PhysicalAddress::new_unchecked(self.as_u64() as usize) }
    }
}

impl IntoPal<PageFrame<Platform>> for x86_64::structures::paging::PhysFrame {
    fn into_pal(self) -> PageFrame<Platform> {
        PageFrame::from_start(self.start_address().into_pal())
    }
}