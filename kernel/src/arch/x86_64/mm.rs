/// Bridge from the platform-specific memory map to the shared memory map types.
pub struct MemoryMap {}

// impl MemoryMap {
//     pub fn iter(&self) -> impl Iterator<Item =
// }

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
            PhysicalAddress::new(r.start),
            PhysicalAddress::new(r.end),
        )
    }
}
