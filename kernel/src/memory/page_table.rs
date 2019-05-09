use bootloader::BootInfo;
use spin::{Mutex, Once};
use x86_64::registers::control::{Cr3, Cr3Flags};
use x86_64::structures::paging::mapper::MapToError;
use x86_64::structures::paging::{
    MappedPageTable, Mapper, MapperAllSizes, Page, PageSize, PageTable, PhysFrame, Size1GiB,
    Size2MiB, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

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
    fn from_active_table(physical_memory_offset: u64) -> PageTableState {
        let (table_frame, _) = Cr3::read();
        let table_addr =
            VirtAddr::new(table_frame.start_address().as_u64() + physical_memory_offset);

        PageTableState {
            physical_memory_offset,
            active_table: unsafe { table_addr.as_mut_ptr::<PageTable>().as_mut() }.unwrap(),
        }
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

    /// Map a single 4KiB page
    pub unsafe fn map_page(
        &mut self,
        frame: PhysFrame<Size4KiB>,
        page: Page<Size4KiB>,
        writable: bool,
    ) -> Result<(), PageTableError> {
        use x86_64::structures::paging::PageTableFlags as Flags;

        let mut flags = Flags::PRESENT;
        if writable {
            flags |= Flags::WRITABLE;
        }

        let mut allocator = crate::memory::frame::page_table_allocator();

        self.active_4kib_mapper()
            .map_to(page, frame, flags, &mut allocator)?
            .flush();

        Ok(())
    }
}

static PAGE_TABLE_STATE: Once<Mutex<PageTableState>> = Once::new();

pub fn init(boot_info: &BootInfo) {
    PAGE_TABLE_STATE.call_once(|| {
        Mutex::new(PageTableState::from_active_table(
            boot_info.physical_memory_offset,
        ))
    });
}

pub fn with_page_table<F, T>(f: F) -> T
where
    F: FnOnce(&mut PageTableState) -> T,
{
    let mut state = PAGE_TABLE_STATE
        .wait()
        .expect("Page table manager not initialized")
        .lock();
    f(&mut *state)
}
