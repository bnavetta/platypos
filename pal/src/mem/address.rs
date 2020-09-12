//! Platform-agnostic address types. These live in the PAL so that PAL APIs (especially for the
//! memory model) can use them.

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::Platform;
use super::MemoryModel;

/// A virtual memory address.
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct VirtualAddress<P: Platform> {
    raw: usize,
    _platform: PhantomData<&'static P>,
}

/// A physical memory address
#[repr(transparent)]
#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysicalAddress<P: Platform> {
    raw: usize,
    _platform: PhantomData<&'static P>
}

//region VirtualAddress

impl <P: Platform> VirtualAddress<P> {
    /// Creates a virtual address without checking the value
    pub const unsafe fn new_unchecked(raw: usize) -> VirtualAddress<P> {
        VirtualAddress { raw, _platform: PhantomData }
    }

    /// Creates a new virtual address.
    ///
    /// # Panics
    /// If the address is invalid. To recover from invalid addresses, use `MemoryModel::virtual_address` instead.
    pub fn new(raw: usize) -> VirtualAddress<P> {
        // TODO: skip validation in release mode?
        P::MemoryModel::virtual_address(raw).unwrap()
    }

    /// Gets the raw address value
    pub const fn into_inner(self) -> usize {
        self.raw
    }

    /// Converts this address to a pointer
    pub fn as_ptr<T>(self) -> *const T {
        self.raw as *const T
    }

    /// Converts this address to a mutable pointer
    pub fn as_mut<T>(self) -> *mut T {
        self.raw as *mut T
    }
}

//endregion

//region VirtualAddress Operators

impl <P: Platform> Add<usize> for VirtualAddress<P> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self::new(self.raw + rhs)
    }
}

impl <P: Platform> AddAssign<usize> for VirtualAddress<P> {
    fn add_assign(&mut self, rhs: usize) {
        // Going threw new to enforce validation
        *self = Self::new(self.raw + rhs)
    }
}

impl <P: Platform> Sub<usize> for VirtualAddress<P> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        Self::new(self.raw - rhs)
    }
}

/// Allows using `-` to get the difference between two addresses
impl <P: Platform> Sub<Self> for VirtualAddress<P> {
    type Output = usize;

    fn sub(self, rhs: Self) -> usize {
        self.raw - rhs.raw
    }
}

impl <P: Platform> SubAssign<usize> for VirtualAddress<P> {
    fn sub_assign(&mut self, rhs: usize) {
        // Going threw new to enforce validation
        *self = Self::new(self.raw - rhs)
    }
}

//endregion

//region VirtualAddress Formatting

// fmt::Debug and fmt::Display are opinionated, then other formatting trait implementations
// delegate to the raw usize so all formatting options work (padding, prefix, etc.)

impl <P: Platform> fmt::Debug for VirtualAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "VirtualAddress({:#x})", self.raw)
    }
}

impl <P: Platform> fmt::Display for VirtualAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.raw)
    }
}

impl <P: Platform> fmt::Octal for VirtualAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.raw, f)
    }
}

impl <P: Platform> fmt::LowerHex for VirtualAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.raw, f)
    }
}

impl <P: Platform> fmt::UpperHex for VirtualAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.raw, f)
    }
}

impl <P: Platform> fmt::Binary for VirtualAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.raw, f)
    }
}

//endregion

//region PhysicalAddress

impl <P: Platform> PhysicalAddress<P> {
    /// Creates a physical address without checking the value
    pub const unsafe fn new_unchecked(raw: usize) -> PhysicalAddress<P> {
        PhysicalAddress { raw, _platform: PhantomData }
    }

    /// Creates a new physical address.
    ///
    /// # Panics
    /// If the address is invalid. To recover from invalid addresses, use `MemoryModel::physical_address` instead.
    pub fn new(raw: usize) -> PhysicalAddress<P> {
        // TODO: skip validation in release mode?
        P::MemoryModel::physical_address(raw).unwrap()
    }

    /// Gets the raw address value
    pub const fn into_inner(self) -> usize {
        self.raw
    }
}

//endregion

//region PhysicalAddress Operators

impl <P: Platform> Add<usize> for PhysicalAddress<P> {
    type Output = Self;

    fn add(self, rhs: usize) -> Self {
        Self::new(self.raw + rhs)
    }
}

impl <P: Platform> AddAssign<usize> for PhysicalAddress<P> {
    fn add_assign(&mut self, rhs: usize) {
        // Going threw new to enforce validation
        *self = Self::new(self.raw + rhs)
    }
}

impl <P: Platform> Sub<usize> for PhysicalAddress<P> {
    type Output = Self;

    fn sub(self, rhs: usize) -> Self {
        Self::new(self.raw - rhs)
    }
}

/// Allows using `-` to get the difference between two addresses
impl <P: Platform> Sub<Self> for PhysicalAddress<P> {
    type Output = usize;

    fn sub(self, rhs: Self) -> usize {
        self.raw - rhs.raw
    }
}

impl <P: Platform> SubAssign<usize> for PhysicalAddress<P> {
    fn sub_assign(&mut self, rhs: usize) {
        // Going threw new to enforce validation
        *self = Self::new(self.raw - rhs)
    }
}

//endregion

//region PhysicalAddress Formatting

// fmt::Debug and fmt::Display are opinionated, then other formatting trait implementations
// delegate to the raw usize so all formatting options work (padding, prefix, etc.)

impl <P: Platform> fmt::Debug for PhysicalAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "PhysicalAddress({:#x})", self.raw)
    }
}

impl <P: Platform> fmt::Display for PhysicalAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.raw)
    }
}

impl <P: Platform> fmt::Octal for PhysicalAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Octal::fmt(&self.raw, f)
    }
}

impl <P: Platform> fmt::LowerHex for PhysicalAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::LowerHex::fmt(&self.raw, f)
    }
}

impl <P: Platform> fmt::UpperHex for PhysicalAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::UpperHex::fmt(&self.raw, f)
    }
}

impl <P: Platform> fmt::Binary for PhysicalAddress<P> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Binary::fmt(&self.raw, f)
    }
}

//endregion
