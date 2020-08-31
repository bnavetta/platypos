use crate::Platform;

mod address;

pub use self::address::{PhysicalAddress, VirtualAddress, ValidateAddressError};

// A platform's memory model. PlatypOS assumes that all platforms use flat address spaces divided
// into fixed-size frames. It also assumes multiple virtual address spaces mapped onto the physical
// address space at page-level granularity.
pub trait MemoryModel<P: Platform>: 'static + Sized {
    /// The size of a frame of physical memory, in bytes.
    const FRAME_SIZE: usize;

    // TODO: how to represent larger page sizes? can probably hide this in the address space API,
    //       let it decide which page size to use

    /// Creates a new physical address out of a raw value, failing if it is invalid
    fn physical_address(raw: usize) -> Result<PhysicalAddress<P>, ValidateAddressError>;

    /// Creates a new virtual address out of a raw value, failing if it is invalid
    fn virtual_address(raw: usize) -> Result<VirtualAddress<P>, ValidateAddressError>;
}