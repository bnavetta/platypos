//! Address space representation

use alloc::sync::Arc;
use core::cell::RefCell;

use hashbrown::HashSet;
use log::debug;
use spin::{Mutex, Once};
use x86_64::structures::paging::{
    FrameAllocator, FrameDeallocator, MappedPageTable, Mapper, MapperAllSizes, Page, PageSize, PageTable, PhysFrame, PageTableFlags
};
use x86_64::structures::paging::mapper::{MapToError, UnmapError};
use x86_64::{PhysAddr, VirtAddr};

use super::{FRAME_SIZE, physical_to_virtual};
use crate::kernel_state;
use crate::memory::page_table::with_active_page_table;
use crate::topology::processor::local_id;
use crate::processor_local;

/// Translator for MappedPageTable
fn page_table_accessor(frame: PhysFrame) -> *mut PageTable {
    physical_to_virtual(frame.start_address()).as_mut_ptr()
}

/// Template page table containing the kernel mappings. All address spaces start with a copy of this
/// page table, so that the kernel is always mapped
static TEMPLATE: Once<&'static PageTable> = Once::new();

// TODO: mark the kernel page table entries as global, so they're not cleared from the TLB

/// Initializes the address space template. This will also switch the current processor over to
/// a new address space based on the template. Doing so makes it safe to overwrite the memory
/// containing the bootloader-provided page tables, which is necessary for multiprocessor
/// initialization (the bootloader puts its page tables in low memory).
///
/// Important: the kernel cannot rely on any mappings made after this point being consistently
/// present. The template address space is bootstrapped from the active page table, but further
/// modifications are not mirrored across address spaces.
pub fn init() {
    TEMPLATE.call_once(|| {
        let (template_table, _) = with_active_page_table(|pt| {
            let page_table_address = pt.borrow().physical_address();
            let page_table = unsafe { &*physical_to_virtual(page_table_address).as_ptr::<PageTable>() };

            debug!("Creating template address space from page table at {:?}", page_table_address);
            clone_pml4(page_table)
        });

        template_table
    });

    unsafe { AddressSpace::switch(Arc::new(AddressSpace::new())); }
}

/// Logical representation of an address space. This is a higher-level wrapper over a raw page table
/// which supports page table modifications in a multiprocessor-safe way.
///
/// Memory for the page tables is managed by the address space. However, the page frames referenced
/// by the page table are not. If a page is mapped into this address space, it must still be freed
/// separately.
pub struct AddressSpace {
    /// The set of processors currently using this address space
    processors: Mutex<HashSet<usize>>,

    /// The physical address of the top-level page table
    pml4_location: PhysAddr,

    /// Reference to the page table backing this address space. The Mutex is to prevent concurrent
    /// modification, but the page table itself isn't stored in this struct. Instead, it's explicitly
    /// allocated and freed via the page frame allocator.
    pml4: Mutex<&'static mut PageTable>,
}

impl AddressSpace {
    /// Create a new address space. This address space will include kernel mappings from the template
    /// page tables.
    pub fn new() -> AddressSpace {
        let (table, location) = clone_pml4(TEMPLATE.wait().expect("Template page table not created"));

        AddressSpace {
            processors: Mutex::new(HashSet::new()),
            pml4_location: location,
            pml4: Mutex::new(table)
        }
    }

    /// Wrap an existing page table in an `AddressSpace`. The caller must ensure that the given
    /// page table is not aliased elsewhere and that it contains all the mappings in the address
    /// space template. In addition, the given `pml4_location` physical address must be the physical
    /// address of the given `pml4` reference.
    ///
    /// # Unsafety
    /// If the given page table does not contain the kernel mappings, switching to it can cause
    /// undefined behavior.
    ///
    /// If the page table reference and physical address do not correspond to each other, then attempting
    /// to modify the page table is also undefined behavior.
    ///
    /// If the page table is aliased, memory safety can be violated.
    pub unsafe fn from_existing(pml4: &'static mut PageTable, pml4_location: PhysAddr) -> AddressSpace {
        AddressSpace {
            processors: Mutex::new(HashSet::new()),
            pml4_location,
            pml4: Mutex::new(pml4)
        }
    }

    /// Get a reference to the active address space on the current processor
    pub fn current() -> Arc<AddressSpace> {
        ACTIVE_ADDRESS_SPACE.with(|a| a.borrow().as_ref().expect("No active address space").clone())
    }

    /// Switch to a different address space.
    ///
    /// # Unsafety
    /// Changing the active address space can violate memory safety.
    pub unsafe fn switch(to: Arc<AddressSpace>) {
        ACTIVE_ADDRESS_SPACE.with(|a| {
            let id = local_id();
            to.on_processor(id);
            let to_pml4 = to.pml4_location;
            if let Some(from) = a.replace(Some(to)) {
                from.off_processor(id);
            }

            with_active_page_table(|pt| pt.borrow_mut().switch(to_pml4));
        });
    }

    /// Add a processor to the set of processors using this address space
    fn on_processor(&self, processor: usize) {
        let mut processors = self.processors.lock();
        processors.insert(processor);
    }

    /// Remove a processor from the set of processors using this address space
    fn off_processor(&self, processor: usize) {
        let mut processors = self.processors.lock();
        processors.remove(&processor);
    }

    /// Translate a virtual address in this address space to the physical address it refers to
    pub fn translate(&self, addr: VirtAddr) -> Option<PhysAddr> {
        let mut pml4 = self.pml4.lock();
        let mapper = unsafe { MappedPageTable::new(&mut pml4, page_table_accessor) };
        mapper.translate_addr(addr)
    }

    /// Map a page to a physical frame.
    ///
    /// # Unsafety
    /// Changing the page tables can cause memory safety violations. In addition, the caller must ensure
    /// that the given frame is not unintentionally mapped into another address space, leading to
    /// invisible aliasing.
    pub unsafe fn map_page(&self, page: Page, frame: PhysFrame, flags: PageTableFlags) -> Result<(), MapToError> {
        // TODO: bulk mapping for more efficient TLB sync
        let mut allocator = KernelFrameAllocator;

        let mut pml4 = self.pml4.lock();
        let mut mapper = MappedPageTable::new(&mut pml4, page_table_accessor);
        mapper.map_to(page, frame, flags, &mut allocator)?.flush();
//        self.with_mapper(|mapper| mapper.map_to(page, frame, flags, &mut allocator))?.flush();

//        unimplemented!("TLB synchronization protocol");
        Ok(())
    }

    /// Unmap a page.
    ///
    /// # Unsafety
    /// Changing the page tables can cause memory safety violations.
    pub unsafe fn unmap_page(&self, page: Page) -> Result<(), UnmapError> {
        let mut pml4 = self.pml4.lock();
        let mut mapper = MappedPageTable::new(&mut pml4, page_table_accessor);
        let (_, flush) = mapper.unmap(page)?;
        flush.flush();

//        unimplemented!("TLB synchronization protocol");
        Ok(())
    }

    /// The physical address of the top-level page table for this address space
    pub fn pml4_location(&self) -> PhysAddr {
        self.pml4_location
    }
}

impl Drop for AddressSpace {
    fn drop(&mut self) {
        // Level 4 - PML4
        let pml4 = self.pml4.lock();

        for entry in pml4.iter() {
            // If it's unused, there's no referenced page table
            if entry.is_unused() || entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                continue;
            }

            // Level 3 - PDPT
            let pdpt_addr = entry.addr();
            let pdpt = unsafe { &*physical_to_virtual(pdpt_addr).as_ptr::<PageTable>() };

            for entry in pdpt.iter() {
                // If it's a 1GiB page, the referenced frame is data, not a page table, and shouldn't be freed
                if entry.is_unused() || entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                    continue;
                }

                // Level 2 - Page Directory
                let pd_addr = entry.addr();
                let pd = unsafe { &*physical_to_virtual(pd_addr).as_ptr::<PageTable>() };

                for entry in pd.iter() {
                    // If it's a 2MiB page, there's also nothing to free
                    if entry.is_unused() || entry.flags().contains(PageTableFlags::HUGE_PAGE) {
                        continue;
                    }

                    // Level 1 - Page Table
                    let pt_addr = entry.addr();
                    kernel_state().frame_allocator().free_at_phys_address(1, pt_addr);
                }

                kernel_state().frame_allocator().free_at_phys_address(1, pd_addr);
            }

            kernel_state().frame_allocator().free_at_phys_address(1, pdpt_addr);
        }

        kernel_state().frame_allocator().free_at_phys_address(1, self.pml4_location);
    }
}

processor_local! {
    static ACTIVE_ADDRESS_SPACE: RefCell<Option<Arc<AddressSpace>>> = RefCell::new(None);
}

/// Implementation of the frame allocation/deallocation abstractions used by MappedPageTable
struct KernelFrameAllocator;

unsafe impl<S: PageSize> FrameAllocator<S> for KernelFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<S>> {
        kernel_state()
            .frame_allocator()
            .allocate_pages(S::SIZE as usize / FRAME_SIZE)
            .map(|alloc| PhysFrame::<S>::from_start_address(alloc.start_phys_address()).unwrap())
    }
}

impl<S: PageSize> FrameDeallocator<S> for KernelFrameAllocator {
    fn deallocate_frame(&mut self, frame: PhysFrame<S>) {
        kernel_state()
            .frame_allocator()
            .free_at_phys_address(S::SIZE as usize / FRAME_SIZE, frame.start_address())
    }
}

/// Makes a deep clone of a Page Map Level 4 (PML4), returning a pointer to the new table and
/// its physical address.
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
pub fn clone_pml4(pml4: &PageTable) -> (&'static mut PageTable, PhysAddr) {
    let allocation = kernel_state().frame_allocator()
        .allocate_pages(1)
        .expect("Could not allocate PML4");

    let new_pml4 = unsafe { &mut *allocation.start_ptr::<PageTable>() };

    // PML4s only contain PDPTs (page directory pointer tables), so we don't have to worry about
    // huge pages _yet_
    for (i, entry) in pml4.iter().enumerate() {
        if entry.is_unused() {
            new_pml4[i].set_unused();
        } else {
            let pdpt = unsafe { &*physical_to_virtual(entry.addr()).as_ptr::<PageTable>() };
            let new_pdpt = clone_pdpt(pdpt);
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
fn clone_pdpt(pdpt: &PageTable) -> PhysAddr {
    let allocation = kernel_state().frame_allocator()
        .allocate_pages(1)
        .expect("Could not allocate PDPT");

    let new_pdpt = unsafe { &mut *allocation.start_ptr::<PageTable>() };

    // PDPTs can reference 1GiB pages, so we have to check for them
    for (i, entry) in pdpt.iter().enumerate() {
        if entry.is_unused() {
            new_pdpt[i].set_unused();
        } else if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Copy the entry exactly, since it points directly at the 1GiB page frame range
            new_pdpt[i] = entry.clone();
        } else {
            let pd = unsafe { &*physical_to_virtual(entry.addr()).as_ptr::<PageTable>() };
            let new_pd = clone_pd(pd);
            new_pdpt[i].set_addr(new_pd, entry.flags());
        }
    }

    allocation.start_phys_address()
}

/// Makes a deep clone of the given page directory, returning the physical address of the new copy
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
fn clone_pd(pd: &PageTable) -> PhysAddr {
    let allocation = kernel_state().frame_allocator()
        .allocate_pages(1)
        .expect("Could not allocate page directory");

    let new_pd = unsafe { &mut *allocation.start_ptr::<PageTable>() };

    // Page directories can reference 2MiB pages
    for (i, entry) in pd.iter().enumerate() {
        if entry.is_unused() {
            new_pd[i].set_unused();
        } else if entry.flags().contains(PageTableFlags::HUGE_PAGE) {
            // Copy exactly, since it points to the 2MiB region directly
            new_pd[i] = entry.clone();
        } else {
            let pt = unsafe { &*physical_to_virtual(entry.addr()).as_ptr::<PageTable>() };
            let new_pt = clone_pt(pt);
            new_pd[i].set_addr(new_pt, entry.flags());
        }
    }

    allocation.start_phys_address()
}

/// Clones the given page table, returning the physical address of the new copy
///
/// # Panics
/// If unable to allocate memory for the page table
fn clone_pt(pt: &PageTable) -> PhysAddr {
    let allocation = kernel_state().frame_allocator()
        .allocate_pages(1)
        .expect("Could not allocate page table");

    let new_pt = unsafe { &mut *allocation.start_ptr::<PageTable>() };

    // Page tables are nice and simple, they point directly at page frames
    for (i, entry) in pt.iter().enumerate() {
        new_pt[i] = entry.clone();
    }

    allocation.start_phys_address()
}
