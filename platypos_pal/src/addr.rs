use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign, Mul, MulAssign, Div, DivAssign};

/// A virtual memory address. This is a wrapper around an `usize`, so it is always sized to the
/// current system's pointer size. It does not enforce platform-specific address requirements.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct VirtualAddress(usize);

impl VirtualAddress {
    pub const fn new(addr: usize) -> VirtualAddress {
        VirtualAddress(addr)
    }

    /// Create a new `VirtualAddress` with the given pointer address
    pub fn from_pointer<T>(p: *const T) -> VirtualAddress {
        VirtualAddress(p as usize)
    }

    pub fn as_pointer<T>(&self) -> *const T {
        self.0 as *const T
    }

    pub unsafe fn as_ref<'a, T>(&self) -> &'a T {
        &* self.as_pointer()
    }

    pub fn as_mut_pointer<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    pub unsafe fn as_mut_ref<'a, T>(&self) -> &'a mut T {
        &mut *self.as_mut_pointer()
    }
}

impl fmt::Display for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::Debug for VirtualAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Virtual({:#x})", self.0)
    }
}

impl From<VirtualAddress> for usize {
    fn from(v: VirtualAddress) -> usize {
        v.0
    }
}

/// A physical memory address. This is a wrapper around an `usize`, so it is always sized to the
///// current system's pointer size. It does not enforce platform-specific address requirements.
#[repr(transparent)]
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub struct PhysicalAddress(usize);

impl PhysicalAddress {
    pub const fn new(addr: usize) -> PhysicalAddress {
        PhysicalAddress(addr)
    }
}

impl fmt::Display for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:#x}", self.0)
    }
}

impl fmt::Debug for PhysicalAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Physical({:#x})", self.0)
    }
}

impl From<PhysicalAddress> for usize {
    fn from(v: PhysicalAddress) -> usize {
        v.0
    }
}

macro_rules! operator_impls {
    ($t:ty) => {
        impl Add<usize> for $t {
            type Output = Self;

            fn add(self, rhs: usize) -> Self {
                <$t>::new(self.0 + rhs)
            }
        }

        impl AddAssign<usize> for $t {
            fn add_assign(&mut self, rhs: usize) {
                self.0 = self.0 + rhs;
            }
        }

        impl Add<Self> for $t {
            type Output = Self;

            fn add(self, rhs: Self) -> Self {
                <$t>::new(self.0 + rhs.0)
            }
        }

        impl AddAssign<Self> for $t {
            fn add_assign(&mut self, rhs: Self) {
                self.0 += rhs.0;
            }
        }

        impl Sub<usize> for $t {
            type Output = Self;

            fn sub(self, rhs: usize) -> Self {
                <$t>::new(self.0 - rhs)
            }
        }

        impl SubAssign<usize> for $t {
            fn sub_assign(&mut self, rhs: usize) {
                self.0 = self.0 - rhs;
            }
        }

        impl Sub<Self> for $t {
            type Output = Self;

            fn sub(self, rhs: Self) -> Self {
                <$t>::new(self.0 - rhs.0)
            }
        }

        impl SubAssign<Self> for $t {
            fn sub_assign(&mut self, rhs: Self) {
                self.0 -= rhs.0;
            }
        }

        impl Mul<usize> for $t {
            type Output = Self;

            fn mul(self, rhs: usize) -> Self {
                <$t>::new(self.0 * rhs)
            }
        }

        impl MulAssign<usize> for $t {
            fn mul_assign(&mut self, rhs: usize) {
                self.0 = self.0 * rhs;
            }
        }

        impl Div<usize> for $t {
            type Output = Self;

            fn div(self, rhs: usize) -> Self {
                <$t>::new(self.0 / rhs)
            }
        }

        impl DivAssign<usize> for $t {
            fn div_assign(&mut self, rhs: usize) {
                self.0 = self.0 / rhs;
            }
        }

        impl $t {
            /// Align down to a power of two
            pub fn align_down(self, to: usize) -> Self {
                debug_assert!(to.is_power_of_two(), "Alignment is not a power of two");
                <$t>::new(self.0 & !(to - 1))
            }

            /// Check if this address is aligned to the given power of two
            pub fn is_aligned(&self, to: usize) -> bool {
                &self.align_down(to) == self
            }
        }
    };
}

operator_impls!(VirtualAddress);
operator_impls!(PhysicalAddress);