//! Make a deep clone of a page table. All levels of page tables are cloned, but the referenced page
//! frames themselves are not.
//!
use x86_64::structures::paging::PageTable;
use x86_64::PhysAddr;

use crate::memory::frame::FrameAllocator;

/// Makes a deep clone of a Page Map Level 4 (PML4), returning a pointer to the new table and
/// its physical address.
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
pub fn clone_pml4(
    allocator: &FrameAllocator,
    physical_memory_offset: u64,
    pml4: &PageTable,
) -> (&'static mut PageTable, PhysAddr) {
    let allocation = allocator
        .allocate_pages(1)
        .expect("Could not allocate PML4");

    let new_pml4 = unsafe { allocation.start_ptr::<PageTable>().as_mut().unwrap() };

    // PML4s only contain PDPTs (page directory pointer tables), so we don't have to worry about
    // huge pages _yet_
    for (i, entry) in pml4.iter().enumerate() {
        if entry.is_unused() {
            new_pml4[i].set_unused();
        } else {
            let pdpt = unsafe {
                ((entry.addr().as_u64() + physical_memory_offset) as *const PageTable)
                    .as_ref()
                    .unwrap()
            };
            let new_pdpt = clone_pdpt(allocator, physical_memory_offset, pdpt);
            new_pml4[i].set_addr(new_pdpt, entry.flags());
        }
    }

    (new_pml4, allocation.start_phys_address())
}

/// Makes a deep clone of the given page directory pointer table, returning the physical address of
/// the new copy.
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
fn clone_pdpt(
    allocator: &FrameAllocator,
    physical_memory_offset: u64,
    pdpt: &PageTable,
) -> PhysAddr {
    let allocation = allocator
        .allocate_pages(1)
        .expect("Could not allocate PDPT");

    let new_pdpt = unsafe { allocation.start_ptr::<PageTable>().as_mut().unwrap() };

    // PDPTs can reference 1GiB pages, so we have to check for them
    for (i, entry) in pdpt.iter().enumerate() {
        if entry.is_unused() {
            new_pdpt[i].set_unused();
        } else if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Copy the entry exactly, since it points directly at the 1GiB page frame range
            new_pdpt[i] = entry.clone();
        } else {
            let pd = unsafe {
                ((entry.addr().as_u64() + physical_memory_offset) as *const PageTable)
                    .as_ref()
                    .unwrap()
            };
            let new_pd = clone_pd(allocator, physical_memory_offset, pd);
            new_pdpt[i].set_addr(new_pd, entry.flags());
        }
    }

    allocation.start_phys_address()
}

/// Makes a deep clone of the given page directory, returning the physical address of the new copy
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
fn clone_pd(allocator: &FrameAllocator, physical_memory_offset: u64, pd: &PageTable) -> PhysAddr {
    let allocation = allocator
        .allocate_pages(1)
        .expect("Could not allocate page directory");

    let new_pd = unsafe { allocation.start_ptr::<PageTable>().as_mut().unwrap() };

    // Page directories can reference 2MiB pages
    for (i, entry) in pd.iter().enumerate() {
        if entry.is_unused() {
            new_pd[i].set_unused();
        } else if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Copy exactly, since it points to the 2MiB region directly
            new_pd[i] = entry.clone();
        } else {
            let pt = unsafe {
                ((entry.addr().as_u64() + physical_memory_offset) as *const PageTable)
                    .as_ref()
                    .unwrap()
            };
            let new_pt = clone_pt(allocator, pt);
            new_pd[i].set_addr(new_pt, entry.flags());
        }
    }

    allocation.start_phys_address()
}

/// Clones the given page table, returning the physical address of the new copy
///
/// # Panics
/// If unable to allocate memory for the page table
fn clone_pt(allocator: &FrameAllocator, pt: &PageTable) -> PhysAddr {
    let allocation = allocator
        .allocate_pages(1)
        .expect("Could not allocate page table");

    let new_pt = unsafe { allocation.start_ptr::<PageTable>().as_mut().unwrap() };

    // Page tables are nice and simple, they point directly at page frames
    for (i, entry) in pt.iter().enumerate() {
        new_pt[i] = entry.clone();
    }

    allocation.start_phys_address()
}
