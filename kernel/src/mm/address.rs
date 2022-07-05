//! Address representations

use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PhysicalAddress(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct VirtualAddress(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct PageFrame(usize);

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Page(usize);

/// Address-like type, which can be used with `AddressRange`
pub trait Address:
    fmt::Display
    + defmt::Format
    + Clone
    + Copy
    + PartialOrd
    + Ord
    + Add<usize, Output = Self>
    + Sub<Self, Output = usize>
    + AddAssign<usize>
    + SubAssign<usize>
{
    const LABEL: &'static str;

    const ZERO: Self;
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct AddressRange<A: Address> {
    /// Starting address (inclusive)
    start: A,
    /// Length in address units
    size: usize,
}

pub type PhysicalAddressRange = AddressRange<PhysicalAddress>;
pub type VirtualAddressRange = AddressRange<VirtualAddress>;
pub type PageFrameRange = AddressRange<PageFrame>;
pub type PageRange = AddressRange<Page>;

/// Behavior for address types (PhysicalAddress and VirtualAddress)
macro_rules! address_ops {
    ($name:ident) => {
        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                // TODO: should the padding depend on the architecture?
                write!(f, "{:#012x}", self.as_usize())
            }
        }

        impl defmt::Format for $name {
            fn format(&self, f: defmt::Formatter) {
                defmt::write!(f, "{=usize:#012x}", self.as_usize())
            }
        }
    };
}

/// Behavior for address-like types, including Page and PageFrame
macro_rules! address_like_ops {
    ($name:ident) => {
        impl From<$name> for usize {
            fn from(a: $name) -> Self {
                a.as_usize()
            }
        }

        impl From<usize> for $name {
            fn from(v: usize) -> Self {
                Self::new(v)
            }
        }

        impl fmt::Debug for $name {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                let name = stringify!($name);
                write!(f, "{}({})", name, self)
            }
        }

        // Always go through `new` so we can potentially put platform-specific
        // assertions there

        impl Add<usize> for $name {
            type Output = $name;

            fn add(self, rhs: usize) -> Self {
                Self::new(self.as_usize() + rhs)
            }
        }

        impl AddAssign<usize> for $name {
            fn add_assign(&mut self, rhs: usize) {
                *self = Self::new(self.as_usize() + rhs);
            }
        }

        impl Sub<usize> for $name {
            type Output = $name;

            fn sub(self, rhs: usize) -> Self {
                Self::new(self.as_usize() - rhs)
            }
        }

        impl SubAssign<usize> for $name {
            fn sub_assign(&mut self, rhs: usize) {
                *self = Self::new(self.as_usize() - rhs);
            }
        }

        impl Sub<$name> for $name {
            type Output = usize;

            fn sub(self, rhs: Self) -> Self::Output {
                self.as_usize() - rhs.as_usize()
            }
        }

        impl Address for $name {
            const LABEL: &'static str = stringify!($name);

            const ZERO: Self = Self::new(0);
        }
    };
}

macro_rules! page_ops {
    ($page:ident, $addr:ident, $desc:literal) => {
        impl $page {
            // TODO: does this translation between pages and addresses hold for all
            // platforms? TODO: is the base size for virtual and physical pages always
            // the same?

            /// The starting address of this page
            pub const fn start(self) -> $addr {
                $addr::new(self.as_usize() * $crate::arch::PAGE_SIZE)
            }

            /// Produce a new page that starts at the given address. If the address is
            /// not correctly aligned, and there for not a valid start, this returns
            /// `Err(())`.
            pub fn from_start(start: $addr) -> Result<Self, ()> {
                let start = start.as_usize();
                if start % $crate::arch::PAGE_SIZE == 0 {
                    Ok(Self::new(start / $crate::arch::PAGE_SIZE))
                } else {
                    Err(())
                }
            }

            /// Returns the page that contains the given address
            pub fn containing(addr: $addr) -> Self {
                Self::new(addr.as_usize() / $crate::arch::PAGE_SIZE)
            }
        }

        impl fmt::Display for $page {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                // TODO: should the padding depend on the architecture?
                write!(f, "{} {:#012x}", $desc, self.as_usize())
            }
        }

        impl defmt::Format for $page {
            fn format(&self, f: defmt::Formatter) {
                let label: defmt::Str = defmt::intern!($desc);
                defmt::write!(f, "{=istr} {=usize:#012x}", label, self.as_usize());
            }
        }

        impl AddressRange<$page> {
            /// The size of this range, in bytes
            pub fn size_bytes(&self) -> usize {
                self.size * $crate::arch::PAGE_SIZE
            }

            /// The starting address of this range
            pub fn start_address(&self) -> $addr {
                self.start.start()
            }

            pub fn address_range(&self) -> AddressRange<$addr> {
                AddressRange::from_start_size(
                    self.start_address(),
                    self.size * $crate::arch::PAGE_SIZE,
                )
            }
        }
    };
}

address_like_ops!(PhysicalAddress);
address_like_ops!(VirtualAddress);
address_ops!(PhysicalAddress);
address_ops!(VirtualAddress);

address_like_ops!(PageFrame);
address_like_ops!(Page);
page_ops!(PageFrame, PhysicalAddress, "PF");
page_ops!(Page, VirtualAddress, "Page");

impl PhysicalAddress {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl VirtualAddress {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl PageFrame {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl Page {
    pub const fn new(address: usize) -> Self {
        Self(address)
    }

    pub const fn as_usize(self) -> usize {
        self.0
    }
}

impl<A: Address> AddressRange<A> {
    // TODO: many of these can be `const` if https://github.com/rust-lang/rust/issues/67792 or const generics are ever stabilized

    pub const fn from_start_size(start: A, size: usize) -> Self {
        Self { start, size }
    }

    pub fn new(start: A, end: A) -> Self {
        assert!(end >= start);
        Self {
            start,
            size: end - start,
        }
    }

    pub const fn empty() -> Self {
        Self {
            start: A::ZERO,
            size: 0,
        }
    }

    /// The starting address of this range (inclusive)
    pub const fn start(&self) -> A {
        self.start
    }

    /// The end address of this range (exclusive)
    pub fn end(&self) -> A {
        self.start + self.size
    }

    /// The size of this range, in whatever units [`A`] indexes.
    pub fn size(&self) -> usize {
        self.size
    }

    pub fn set_size(&mut self, size: usize) {
        self.size = size;
    }

    /// Shrink this range by removing `amount` units from the left (and thus
    /// adding `amount` units to the start address)
    pub fn shrink_left(&mut self, amount: usize) {
        self.size -= amount;
        self.start += amount;
    }

    /// Shrink this range by removing `amount` units from the right (reducing
    /// the size but not changing the start address)
    pub fn shrink_right(&mut self, amount: usize) {
        self.size -= amount;
    }

    /// Extend this range by adding `amount` units to the left (and thus
    /// subtracting `amount` units from the start address)
    pub fn extend_left(&mut self, amount: usize) {
        self.size += amount;
        self.start -= amount;
    }

    /// Extend this range by adding `amount` units to the right (increasing the
    /// size but not changing the start address)
    pub fn extend_right(&mut self, amount: usize) {
        self.size += amount;
    }

    /// Tests if this range completely contains `other`
    pub fn contains(&self, other: &Self) -> bool {
        self.start <= other.start && self.end() >= other.end()
    }

    /// Tests if this range overlaps at all with `other`
    pub fn intersects(&self, other: &Self) -> bool {
        self.start <= other.end() && self.end() >= other.start
    }
}

// TODO: Tests

impl<A: Address> fmt::Display for AddressRange<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} - {}", self.start, self.end())
    }
}

impl<A: Address> fmt::Debug for AddressRange<A> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}Range({} - {})", A::LABEL, self.start, self.end())
    }
}

impl<A: Address> defmt::Format for AddressRange<A> {
    fn format(&self, f: defmt::Formatter) {
        defmt::write!(f, "{=str}Range({} - {})", A::LABEL, self.start, self.end());
    }
}
