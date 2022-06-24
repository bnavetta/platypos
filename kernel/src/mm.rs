use core::fmt;
use core::ops::Sub;

pub mod map;
pub mod physical;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // TODO: should the padding depend on the architecture?
        write!(f, "{:#012x}", self.0)
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PhysicalAddress({})", self)
    }
}

impl From<usize> for PhysicalAddress {
    fn from(address: usize) -> Self {
        Self::new(address)
    }
}

impl From<PhysicalAddress> for usize {
    fn from(addr: PhysicalAddress) -> Self {
        addr.as_usize()
    }
}

impl Sub<PhysicalAddress> for PhysicalAddress {
    type Output = usize;

    fn sub(self, rhs: PhysicalAddress) -> Self::Output {
        self.0 - rhs.0
    }
}

/// Wrapper for human-readable byte sizes
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ByteSize(usize);

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

pub trait ByteSizeExt {
    fn as_size(&self) -> ByteSize;
}

impl ByteSizeExt for usize {
    #[inline(always)]
    fn as_size(&self) -> ByteSize {
        ByteSize(*self)
    }
}
