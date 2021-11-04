//! Memory map representation

use alloc::vec::Vec;
use core::fmt;

use bitflags::bitflags;

use crate::arch::address::PhysicalAddress;

#[derive(Debug, Clone)]
pub struct MemoryMap {
    regions: Vec<Region>,
}

/// A region of the physical address space
#[derive(Debug, Clone)]
pub struct Region {
    /// The starting address of this region (inclusive)
    pub start: PhysicalAddress,
    /// The ending address of this region (exclusive)
    pub end: PhysicalAddress,
    pub kind: Kind,
    pub flags: Flags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// Conventional random-access memory
    Ram,
    /// Memory-mapped I/O region
    Mmio,
}

bitflags! {
    pub struct Flags: u8 {
        /// This region contains the kernel
        const KERNEL =   0b00000001;
        /// This region is reserved by the firmware
        const FIRMWARE = 0b00000010;

        // TODO: flags for cacheability, etc.

        /// Mask to check if a region contains allocatable memory (i.e. it's not already in use by the system)
        const ALLOCATABLE = !(Self::KERNEL.bits | Self::FIRMWARE.bits);
    }
}

impl MemoryMap {
    pub fn new(mut regions: Vec<Region>) -> MemoryMap {
        regions.sort_unstable_by(|a, b| a.start.cmp(&b.start));

        for pair in regions.windows(2) {
            match pair {
                [a, b] => {
                    if a.end > b.start {
                        panic!("Overlapping memory regions:\n- {}\n- {}", a, b);
                    }
                }
                _ => unreachable!(),
            }
        }

        MemoryMap { regions }
    }

    /// Determines the lowest and highest addresses of allocatable RAM. Not
    /// every part of this range is usable, but it bounds the parts of the
    /// physical address space which the memory allocation system must manage.
    pub fn allocatable_ram_range(&self) -> (PhysicalAddress, PhysicalAddress) {
        // Regions are sorted, so we can just linearly scan through them

        let low = self
            .regions
            .iter()
            .find_map(|r| {
                if r.kind == Kind::Ram && Flags::ALLOCATABLE.contains(r.flags) {
                    Some(r.start)
                } else {
                    None
                }
            })
            .expect("no usable RAM");

        let high = self
            .regions
            .iter()
            .rev()
            .find_map(|r| {
                if r.kind == Kind::Ram && Flags::ALLOCATABLE.contains(r.flags) {
                    Some(r.end)
                } else {
                    None
                }
            })
            .expect("no usable RAM");

        (low, high)
    }
}

impl fmt::Display for MemoryMap {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for region in self.regions.iter() {
            writeln!(f, "* {}", region)?;
        }
        Ok(())
    }
}

impl Region {
    /// The size of this region, in bytes
    pub fn size_bytes(&self) -> usize {
        self.end - self.start
    }
}

impl fmt::Display for Region {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}-{}: {}, {}",
            self.start,
            self.end,
            self.kind,
            ByteSize(self.size_bytes())
        )?;

        if !self.flags.is_empty() {
            write!(f, " [{:?}]", self.flags)?;
        }

        Ok(())
    }
}

impl fmt::Display for Kind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Kind::Ram => "RAM",
            Kind::Mmio => "MMIO",
        })
    }
}

struct ByteSize(usize);

impl ByteSize {
    const GiB: usize = 1024 * ByteSize::MiB;
    const MiB: usize = 1024 * ByteSize::KiB;
    const KiB: usize = 1024;
}

impl fmt::Display for ByteSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let gigabytes = self.0 / ByteSize::GiB;
        let rem_gb = self.0 % ByteSize::GiB;

        if gigabytes > 0 {
            write!(f, "{} GiB", gigabytes)?;
            if rem_gb > 0 {
                write!(f, " + ")?;
            }
        }

        let megabytes = rem_gb / ByteSize::MiB;
        let rem_mb = rem_gb % ByteSize::MiB;

        if megabytes > 0 {
            write!(f, "{} MiB", megabytes)?;
            if rem_mb > 0 {
                write!(f, " + ")?;
            }
        }

        let kilobytes = rem_mb / ByteSize::KiB;
        let bytes = rem_mb % ByteSize::KiB;

        if kilobytes > 0 {
            write!(f, "{} KiB", kilobytes)?;
            if bytes > 0 {
                write!(f, " + ")?;
            }
        }

        if bytes > 0 {
            write!(f, "{} bytes", bytes)?;
        }

        Ok(())
    }
}
