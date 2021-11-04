//! Memory address representation

use core::{fmt, ops};

/// A memory address in the system's physical address space.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PhysicalAddress(usize);

/// Identifier for a page of physical memory.
#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
// Represent physical pages by their starting address - the bits wouldn't be
// used for anything else otherwise
pub struct PhysicalPage(usize);

impl PhysicalAddress {
    pub const fn new(address: usize) -> PhysicalAddress {
        PhysicalAddress(address)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl ops::Add<usize> for PhysicalAddress {
    type Output = PhysicalAddress;

    fn add(self, rhs: usize) -> Self::Output {
        PhysicalAddress(self.0 + rhs)
    }
}

impl ops::Sub<PhysicalAddress> for PhysicalAddress {
    type Output = usize;

    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        self.0 - rhs.0
    }
}

impl fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#0x}", self.0)
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("PhysicalAddress")
            .field(&format_args!("{:#0x}", self.0))
            .finish()
    }
}

impl PhysicalPage {
    /// The size of a physical page. Because physical memory access is more or
    /// less contiguous, this mostly only matters in the context of virtual
    /// memory management.
    pub const PAGE_SIZE: usize = 4096;

    pub const fn from_page_number(ppn: usize) -> PhysicalPage {
        PhysicalPage(ppn * PhysicalPage::PAGE_SIZE)
    }

    /// The physical page that contains `addr`
    pub const fn containing_address(addr: PhysicalAddress) -> PhysicalPage {
        PhysicalPage(addr.0 & !(PhysicalPage::PAGE_SIZE - 1))
    }

    pub const fn start_address(self) -> PhysicalAddress {
        PhysicalAddress(self.0)
    }

    /// The physical page number (PPN)
    pub const fn page_number(self) -> usize {
        self.0 / PhysicalPage::PAGE_SIZE
    }
}

impl ops::Add<usize> for PhysicalPage {
    type Output = Self;

    /// Adds `rhs` pages to `self`
    fn add(self, rhs: usize) -> Self::Output {
        PhysicalPage(self.0 + rhs * PhysicalPage::PAGE_SIZE)
    }
}

impl fmt::Display for PhysicalPage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PPN {:#0x}", self.page_number())
    }
}

impl fmt::Debug for PhysicalPage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("PhysicalPage")
            .field(&format_args!("{:#0x}", self.page_number()))
            .finish()
    }
}
