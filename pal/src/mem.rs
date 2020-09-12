use core::fmt;

use crate::Platform;

mod address;
mod frame;

pub use self::address::{PhysicalAddress, VirtualAddress};
pub use self::frame::PageFrame;

// A platform's memory model. PlatypOS assumes that all platforms use flat address spaces divided
// into fixed-size frames. It also assumes multiple virtual address spaces mapped onto the physical
// address space at page-level granularity.
pub trait MemoryModel<P: Platform>: 'static + Sized {
    /// The size of a frame of physical memory, in bytes.
    const FRAME_SIZE: usize;

    // TODO: how to represent larger page sizes? can probably hide this in the address space API,
    //       let it decide which page size to use
    //       for physical memory, allocate in units of the minimum page size since that's the smallest
    //       that can be mapped. callers should just ask for however many frames they need

    /// Creates a new physical address out of a raw value, failing if it is invalid
    fn physical_address(raw: usize) -> Result<PhysicalAddress<P>, ValidateAddressError>;

    /// Creates a new virtual address out of a raw value, failing if it is invalid
    fn virtual_address(raw: usize) -> Result<VirtualAddress<P>, ValidateAddressError>;
}

/// An error validating a memory address
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidateAddressError {
    kind: AddressErrorKind,
    raw: usize
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum AddressErrorKind {
    InvalidPhysicalAddress,
    InvalidVirtualAddress,
    NotPageAligned,
}

impl ValidateAddressError {
    fn new(kind: AddressErrorKind, raw: usize) -> ValidateAddressError {
        ValidateAddressError { kind, raw }
    }

    /// Creates a new `ValidateAddressError` indicating that `raw` is not a valid virtual address.
    pub fn invalid_virtual_address(raw: usize) -> ValidateAddressError {
        ValidateAddressError::new(AddressErrorKind::InvalidVirtualAddress, raw)
    }

    /// Creates a new `ValidateAddressError` indicating that `raw` is not a valid physical address.
    pub fn invalid_physical_address(raw: usize) -> ValidateAddressError {
        ValidateAddressError::new(AddressErrorKind::InvalidPhysicalAddress, raw)
    }

    /// The raw address value
    pub fn raw_address(&self) -> usize {
        self.raw
    }
}

impl fmt::Display for ValidateAddressError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.kind {
            AddressErrorKind::InvalidPhysicalAddress => write!(f, "not a valid physical address: {:#x}", self.raw),
            AddressErrorKind::InvalidVirtualAddress => write!(f, "not a valid virtual address: {:#x}", self.raw),
            AddressErrorKind::NotPageAligned => write!(f, "not page-aligned: {:#x}", self.raw)
        }
    }
}
