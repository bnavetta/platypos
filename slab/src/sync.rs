#[cfg(loom)]
pub(crate) use loom::cell::UnsafeCell;

#[cfg(not(loom))]
pub(crate) use core::cell::UnsafeCell;
