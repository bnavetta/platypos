use core::fmt;

mod address;
mod heap_allocator;
pub mod map;
pub mod root_allocator;

pub use self::address::*;

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
