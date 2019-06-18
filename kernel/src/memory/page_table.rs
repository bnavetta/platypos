use core::cell::{RefCell, UnsafeCell};

use x86_64::{PhysAddr, VirtAddr};
use x86_64::structures::paging::{Page, PhysFrame, Size4KiB};
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::instructions::tlb;

use crate::processor_local;

// The two flags in CR3 control caching behavior for the level 4 page table. The defaults are what
// we want.
const CR3_FLAGS: Cr3Flags = Cr3Flags::empty();

/// Maximum size of an invalidated virtual memory region to flush with invlpg instead of a CR3 reload
const INVLPG_MAX_SIZE: u64 = 2 * 1024 * 1024;

#[derive(Debug)]
pub struct ActivePageTable {
    /// Physical address of the active level 4 page table
    /// This is an `UnsafeCell` to ensure that `ActivePageTable` is not `Send` or `Sync`. Since
    /// `ActivePageTable` is only meaningful for the processor it was created on, it should not be
    /// sent or shared between threads.
    phys_addr: UnsafeCell<PhysAddr>,
}

/// Handle to the current processor's active page table. Only valid for the processor it was created
/// on and until the active page table is changed.
impl ActivePageTable {
    /// Switch to a different level 4 page table
    ///
    /// The new page table _must_ contain the kernel's mappings.
    ///
    /// # Unsafety
    /// Changing the level 4 page table is unsafe, since altering the page mapping can violate
    /// memory safety.
    pub unsafe fn switch(&mut self, pml4_location: PhysAddr) {
        let frame = PhysFrame::from_start_address(pml4_location).expect("Page tables must be page-aligned");

        Cr3::write(frame, CR3_FLAGS);
        self.phys_addr = UnsafeCell::new(pml4_location);
    }

    /// Get a handle to the current level 4 page table.
    ///
    /// # Unsafety
    ///
    /// The caller must ensure that there are no other references to the active page table on the
    /// current processor.
    unsafe fn from_current() -> ActivePageTable {
        let (table_frame, _) = Cr3::read();

        let phys_addr = UnsafeCell::new(table_frame.start_address());

        ActivePageTable { phys_addr }
    }

    /// The physical address of the active page table
    pub fn physical_address(&self) -> PhysAddr {
        unsafe { *self.phys_addr.get() }
    }

    /// Flush a range from the TLB on the current processor. All pages between `start` and `end`
    /// (inclusive) will be removed from the TLB.
    pub fn invalidate_tlb(&mut self, start: VirtAddr, end: VirtAddr) {
        if end.as_u64() - start.as_u64() > INVLPG_MAX_SIZE {
            tlb::flush_all();
        } else {
            let start_page = Page::<Size4KiB>::containing_address(start);
            let end_page = Page::<Size4KiB>::containing_address(end);
            for page in Page::range_inclusive(start_page, end_page) {
                tlb::flush(page.start_address());
            }
        }
    }
}

processor_local! {
    static ACTIVE_PAGE_TABLE: RefCell<ActivePageTable> = RefCell::new(unsafe { ActivePageTable::from_current() });
}

/// Access the active page table for the current processor.
pub fn with_active_page_table<F, R>(f: F) -> R where F: FnOnce(&RefCell<ActivePageTable>) -> R {
    ACTIVE_PAGE_TABLE.with(f)
}