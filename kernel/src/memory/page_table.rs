use bootloader::BootInfo;
use log::info;
use x86_64::structures::paging::frame::PhysFrameRange;
use x86_64::structures::paging::page::PageRange;
use x86_64::{
    registers::control::{Cr3, Cr3Flags},
    structures::paging::{
        mapper::MapToError, MappedPageTable, Mapper, MapperAllSizes, PageTable, PageTableFlags,
        PhysFrame, Size1GiB, Size2MiB, Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::memory::frame::FrameAllocator;
use crate::kernel_state;

#[derive(Debug)]
pub enum PageTableError {
    /// The page table address was not page-aligned. Contains the address of the page table
    UnalignedPageTable(PhysAddr),

    /// The virtual address was not in the page table. Contains the unmapped virtual address
    AddressNotMapped(VirtAddr),

    /// There was an error mapping a page
    MappingFailed(MapToError),
}

impl From<MapToError> for PageTableError {
    fn from(err: MapToError) -> PageTableError {
        PageTableError::MappingFailed(err)
    }
}

pub struct PageTableState {
    physical_memory_offset: u64,

    // 'static might not be quite right, but assuming the active page table is only ever
    // changed through PageTableState I think it should be fine
    active_table: &'static mut PageTable,
}

impl PageTableState {
    /// Initialize the page table manager. This will make a copy of the bootloader page tables and
    /// switch to it.
    pub fn initialize(allocator: &FrameAllocator, boot_info: &BootInfo) -> PageTableState {
        let (current_frame, flags) = Cr3::read();
        let current_table_addr = VirtAddr::new(current_frame.start_address().as_u64() + boot_info.physical_memory_offset);
        let current_pml4 = unsafe { current_table_addr.as_mut_ptr::<PageTable>().as_mut().unwrap() };

        let (pml4, pml4_addr) = clone_pml4(allocator, boot_info.physical_memory_offset, current_pml4);

        info!("Switching to new kernel page tables");
        unsafe { Cr3::write(PhysFrame::from_start_address(pml4_addr).unwrap(), flags); }

        PageTableState {
            physical_memory_offset: boot_info.physical_memory_offset,
            active_table: pml4
        }
    }

    /// Returns the virtual address of the current Page-Map Level 4 table
    pub fn current_pml4_address(&mut self) -> VirtAddr {
        VirtAddr::from_ptr(self.active_table)
    }

    /// Returns the physical address of the current Page-Map Level 4 table
    pub fn current_pml4_location(&self) -> PhysAddr {
        Cr3::read().0.start_address()
    }

    pub unsafe fn activate_table(
        &mut self,
        table: &'static mut PageTable,
    ) -> Result<(), PageTableError> {
        let phys_table = self.translate(VirtAddr::from_ptr(table))?;
        let frame = PhysFrame::from_start_address(phys_table)
            .map_err(|_| PageTableError::UnalignedPageTable(phys_table))?;
        Cr3::write(frame, Cr3Flags::empty());
        self.active_table = table;
        Ok(())
    }

    /// Get a MapperAllSizes implementation for the currently-active page table. It's not
    /// at all safe to hold on tho the returned mapper, since it's reliant on the current
    /// page table not changing.
    pub unsafe fn active_mapping<'a>(&'a mut self) -> impl MapperAllSizes + 'a {
        let physical_memory_offset = self.physical_memory_offset;
        MappedPageTable::new(
            self.active_table,
            move |frame: PhysFrame| -> *mut PageTable {
                VirtAddr::new(frame.start_address().as_u64() + physical_memory_offset).as_mut_ptr()
            },
        )
    }

    // Because of how the traits are defined, the MapperAllSizes impl above can only map 1 GiB pages,
    // and because of how MappedPageTable is defined, there can't be an active_mapping method that's
    // generic over PageSize :(

    pub unsafe fn active_4kib_mapper<'a>(&'a mut self) -> impl Mapper<Size4KiB> + 'a {
        let physical_memory_offset = self.physical_memory_offset;
        MappedPageTable::new(
            self.active_table,
            move |frame: PhysFrame| -> *mut PageTable {
                VirtAddr::new(frame.start_address().as_u64() + physical_memory_offset).as_mut_ptr()
            },
        )
    }

    pub unsafe fn active_2mib_mapper<'a>(&'a mut self) -> impl Mapper<Size2MiB> + 'a {
        let physical_memory_offset = self.physical_memory_offset;
        MappedPageTable::new(
            self.active_table,
            move |frame: PhysFrame| -> *mut PageTable {
                VirtAddr::new(frame.start_address().as_u64() + physical_memory_offset).as_mut_ptr()
            },
        )
    }

    pub unsafe fn active_1gib_mapper<'a>(&'a mut self) -> impl Mapper<Size1GiB> + 'a {
        let physical_memory_offset = self.physical_memory_offset;
        MappedPageTable::new(
            self.active_table,
            move |frame: PhysFrame| -> *mut PageTable {
                VirtAddr::new(frame.start_address().as_u64() + physical_memory_offset).as_mut_ptr()
            },
        )
    }

    // Helpers for common tasks

    /// Translate a virtual address to a physical address using the current page table.
    pub fn translate(&mut self, addr: VirtAddr) -> Result<PhysAddr, PageTableError> {
        unsafe {
            self.active_mapping()
                .translate_addr(addr)
                .ok_or_else(|| PageTableError::AddressNotMapped(addr))
        }
    }

    /// Get the virtual address referring to the given physical address in the kernel's physical
    /// memory mapping.
    pub fn physical_map_address(&self, addr: PhysAddr) -> VirtAddr {
        VirtAddr::new(addr.as_u64() + self.physical_memory_offset)
    }

    /// Map a contiguous range of physical memory into the current address space
    pub unsafe fn map_contiguous(
        &mut self,
        pages: PageRange,
        frames: PhysFrameRange,
        writable: bool,
    ) -> Result<(), PageTableError> {
        let mut mapper = self.active_4kib_mapper();
        let mut allocator = kernel_state().frame_allocator().page_table_allocator();

        let flags = PageTableFlags::PRESENT
            | if writable {
                PageTableFlags::WRITABLE
            } else {
                PageTableFlags::empty()
            };

        for (page, frame) in pages.into_iter().zip(frames.into_iter()) {
            mapper.map_to(page, frame, flags, &mut allocator)?.flush();
        }

        Ok(())
    }
}

// TODO: set GLOBAL flag on kernel page table entries so they don't get flushed when switching address spaces?

/// Makes a deep clone of a Page Map Level 4 (PML4), returning a pointer to the new table and
/// its physical address.
///
/// # Panics
/// If unable to allocate memory for any of the needed tables
fn clone_pml4(allocator: &FrameAllocator, physical_memory_offset: u64, pml4: &PageTable) -> (&'static mut PageTable, PhysAddr) {
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
            let pdpt = unsafe { ((entry.addr().as_u64() + physical_memory_offset) as *const PageTable).as_ref().unwrap() };
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
fn clone_pdpt(allocator: &FrameAllocator, physical_memory_offset: u64, pdpt: &PageTable) -> PhysAddr {
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
            let pd = unsafe { ((entry.addr().as_u64() + physical_memory_offset) as *const PageTable).as_ref().unwrap() };
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
            let pt = unsafe { ((entry.addr().as_u64() + physical_memory_offset) as *const PageTable).as_ref().unwrap() };
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