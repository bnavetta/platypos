//! Unified memory map types

use core::fmt;

use crate::prelude::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum Kind {
    /// Conventional, usable memory
    Usable,
    /// Memory reserved by the firmware or bootloader - likely conventional, but
    /// unusable. For example, this may contain the firmware code or the
    /// kernel.
    Reserved,
    /// Memory that contains ACPI tables, which may be reused once the tables
    /// are no longer needed. Only present on systems using ACPI.
    AcpiTables,
    /// Non-volatile ACPI memory. Only present on systems using ACPI.
    AcpiNonVolatile,
    /// Memory that persists across reboots
    Persistent,
    /// An unmapped UEFI memory kind - treat as unusable. Only present on
    /// systems using UEFI.
    Uefi(UefiMemoryKind),
    /// An unmapped legacy PC BIOS memory kind - treat as unusable. Only present
    /// on systems using BIOS.
    Bios(BiosMemoryKind),
}

pub type UefiMemoryKind = u32;
pub type BiosMemoryKind = u32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Region {
    kind: Kind,
    start: PhysicalAddress,
    end: PhysicalAddress,
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(usize);

impl Region {
    pub const fn new(kind: Kind, start: PhysicalAddress, end: PhysicalAddress) -> Self {
        Region { kind, start, end }
    }

    /// The memory kind of this region.
    pub fn kind(&self) -> Kind {
        self.kind
    }

    /// Checks if this region is usable
    pub fn usable(&self) -> bool {
        self.kind == Kind::Usable
    }

    /// The starting address of this region (inclusive)
    pub fn start(&self) -> PhysicalAddress {
        self.start
    }

    /// The ending address of this region (exclusive)
    pub fn end(&self) -> PhysicalAddress {
        self.end
    }

    pub fn size(&self) -> usize {
        (self.end - self.start) as usize
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{:#012x} - {:#012x} {:?} ({})",
            self.start,
            self.end,
            self.kind,
            ByteSize(self.size())
        )
    }
}

const SIZE_UNITS: &[(usize, &str)] = &[
    (1024 * 1024 * 1024, "GiB"),
    (1024 * 1024, "MiB"),
    (1024, "KiB"),
];

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut remaining = self.0;
        let mut needs_spacing = false;
        for (size, suffix) in SIZE_UNITS {
            let amount = remaining / size;
            remaining = remaining % size;

            if amount > 0 {
                if needs_spacing {
                    write!(f, " ")?;
                }
                write!(f, "{} {}", amount, suffix)?;
                needs_spacing = true;
            }
        }

        if remaining > 0 {
            if needs_spacing {
                write!(f, " ")?;
            }
            write!(f, "{} bytes", remaining)?;
        }
        Ok(())
    }
}
