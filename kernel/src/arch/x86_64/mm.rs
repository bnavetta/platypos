use bootloader::boot_info::{MemoryRegion, MemoryRegionKind};

use crate::mm::map::{Kind, Region};
use crate::prelude::*;

use super::PhysicalPageNumber;

impl From<&MemoryRegion> for Region {
    fn from(r: &MemoryRegion) -> Self {
        let kind = match r.kind {
            MemoryRegionKind::Usable => Kind::Usable,
            MemoryRegionKind::Bootloader => Kind::Reserved,
            MemoryRegionKind::UnknownUefi(typ) => Kind::Uefi(typ),
            MemoryRegionKind::UnknownBios(typ) => Kind::Bios(typ),
            _ => Kind::Reserved,
        };

        Region::new(
            kind,
            PhysicalAddress::new(r.start.try_into().unwrap()),
            PhysicalAddress::new(r.end.try_into().unwrap()),
        )
    }
}

/// Accessor for physical memory. The kernel cannot assume that physical memory
/// is mapped into its address space. Instead, it uses this type to create
/// temporary or permanent mappings.
pub struct PhysicalMemoryAccess {
    // On x86_64, we can map all physical memory
    base: *mut u8,
}

impl PhysicalMemoryAccess {
    /// Permanently map `count` pages of physical memory starting at `start`
    /// into the kernel's address space. On success, returns a usable pointer to
    /// the new mapping.
    ///
    /// # Safety
    /// The caller is responsible for not aliasing memory by mapping the same
    /// (or overlapping) physical region twice.
    pub unsafe fn map_permanent(
        &mut self,
        start: PhysicalPageNumber,
        count: usize,
    ) -> Result<*mut u8, Error> {
        // No-op because all memory is already mapped
        let start_offset: isize = start
            .start_address()
            .as_u64()
            .try_into()
            .map_err(|_| Error::new(ErrorKind::AddressOutOfBounds))?;
        Ok(self.base.offset(start_offset))
    }
}
