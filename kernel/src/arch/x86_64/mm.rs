use core::mem::MaybeUninit;
use core::slice;

use bootloader::boot_info::{MemoryRegion, MemoryRegionKind};

use crate::mm::map::{Kind, Region};
use crate::prelude::*;

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
pub struct MemoryAccess {
    // On x86_64, we can map all physical memory
    base: *mut MaybeUninit<u8>,
}

impl MemoryAccess {
    pub(super) unsafe fn new(base: *mut MaybeUninit<u8>) -> Self {
        Self { base }
    }

    /// Temporarily maps `range` into the kernel's address space. The given
    /// function is provided a reference to the mapped region as a mutable
    /// slice.
    ///
    /// # Safety
    /// The caller is responsible for not aliasing memory by mapping the same
    /// (or overlapping) physical region twice.
    ///
    /// The mapping is only valid for the duration of `f` (the lifetime of the
    /// slice). Using the mapping outside of that lifetime is undefined
    /// behavior.
    pub unsafe fn with_memory<F, T>(&mut self, range: PageFrameRange, f: F) -> Result<T, Error>
    where
        F: FnOnce(&mut [MaybeUninit<u8>]) -> T,
    {
        let base = self.map_permanent(range)?;
        let length = range.size() * PAGE_SIZE;
        let slice = slice::from_raw_parts_mut(base, length);
        Ok(f(slice))
    }

    /// Permanently map `range`
    /// into the kernel's address space. On success, returns a usable pointer to
    /// the new mapping.
    ///
    /// # Safety
    /// The caller is responsible for not aliasing memory by mapping the same
    /// (or overlapping) physical region twice.
    pub unsafe fn map_permanent(
        &mut self,
        range: PageFrameRange,
    ) -> Result<*mut MaybeUninit<u8>, Error> {
        // No-op because all memory is already mapped

        let start_offset: isize = range
            .start_address()
            .as_usize()
            .try_into()
            .map_err(|_| Error::new(ErrorKind::AddressOutOfBounds))?;
        Ok(self.base.offset(start_offset))
    }
}
