//! Make a deep clone of a page table. All levels of page tables are cloned, but the referenced page
//! frames themselves are not.
//!
use x86_64::structures::paging::{FrameAllocator, PageTable, PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use crate::util::allocator::UefiPageAllocator;

/// Makes a deep clone of a Page Map Level 4 (PML4), returning a pointer to the new table and
/// its physical address.
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
pub fn clone_pml4(
    allocator: &mut UefiPageAllocator,
    pml4: &PageTable,
) -> (&'static mut PageTable, PhysAddr) {
    let allocation: PhysFrame<Size4KiB> =
        allocator.allocate_frame().expect("Could not allocate PML4");

    let new_pml4: &mut PageTable =
        unsafe { &mut *VirtAddr::new(allocation.start_address().as_u64()).as_mut_ptr() };

    // PML4s only contain PDPTs (page directory pointer tables), so we don't have to worry about
    // huge pages _yet_
    for (i, entry) in pml4.iter().enumerate() {
        if entry.is_unused() {
            new_pml4[i].set_unused();
        } else {
            let pdpt = unsafe {
                (entry.addr().as_u64() as *const PageTable)
                    .as_ref()
                    .unwrap()
            };
            let new_pdpt = clone_pdpt(allocator, pdpt);
            new_pml4[i].set_addr(new_pdpt, entry.flags());
        }
    }

    (new_pml4, allocation.start_address())
}

/// Makes a deep clone of the given page directory pointer table, returning the physical address of
/// the new copy.
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
fn clone_pdpt(allocator: &mut UefiPageAllocator, pdpt: &PageTable) -> PhysAddr {
    let allocation: PhysFrame = allocator.allocate_frame().expect("Could not allocate PDPT");

    let new_pdpt: &mut PageTable =
        unsafe { &mut *VirtAddr::new(allocation.start_address().as_u64()).as_mut_ptr() };

    // PDPTs can reference 1GiB pages, so we have to check for them
    for (i, entry) in pdpt.iter().enumerate() {
        if entry.is_unused() {
            new_pdpt[i].set_unused();
        } else if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Copy the entry exactly, since it points directly at the 1GiB page frame range
            new_pdpt[i] = entry.clone();
        } else {
            let pd = unsafe {
                (entry.addr().as_u64() as *const PageTable)
                    .as_ref()
                    .unwrap()
            };
            let new_pd = clone_pd(allocator, pd);
            new_pdpt[i].set_addr(new_pd, entry.flags());
        }
    }

    allocation.start_address()
}

/// Makes a deep clone of the given page directory, returning the physical address of the new copy
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
fn clone_pd(allocator: &mut UefiPageAllocator, pd: &PageTable) -> PhysAddr {
    let allocation: PhysFrame = allocator
        .allocate_frame()
        .expect("Could not allocate page directory");

    let new_pd: &mut PageTable =
        unsafe { &mut *VirtAddr::new(allocation.start_address().as_u64()).as_mut_ptr() };

    // Page directories can reference 2MiB pages
    for (i, entry) in pd.iter().enumerate() {
        if entry.is_unused() {
            new_pd[i].set_unused();
        } else if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Copy exactly, since it points to the 2MiB region directly
            new_pd[i] = entry.clone();
        } else {
            let pt = unsafe {
                (entry.addr().as_u64() as *const PageTable)
                    .as_ref()
                    .unwrap()
            };
            let new_pt = clone_pt(allocator, pt);
            new_pd[i].set_addr(new_pt, entry.flags());
        }
    }

    allocation.start_address()
}

/// Clones the given page table, returning the physical address of the new copy
///
/// # Panics
/// If unable to allocate memory for the page table
fn clone_pt(allocator: &mut UefiPageAllocator, pt: &PageTable) -> PhysAddr {
    let allocation: PhysFrame = allocator
        .allocate_frame()
        .expect("Could not allocate page table");

    let new_pt: &mut PageTable =
        unsafe { &mut *VirtAddr::new(allocation.start_address().as_u64()).as_mut_ptr() };

    // Page tables are nice and simple, they point directly at page frames
    for (i, entry) in pt.iter().enumerate() {
        new_pt[i] = entry.clone();
    }

    allocation.start_address()
}
