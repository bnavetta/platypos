//! Kernel synchronization primitives

use core::fmt;
use core::ops::{Deref, DerefMut};

use spin::{Mutex, MutexGuard};

use crate::arch::interrupts;

pub struct InterruptSafeMutex<T: ?Sized> {
    inner: Mutex<T>,
}

pub struct InterruptSafeMutexGuard<'a, T: ?Sized> {
    // The order of these fields is important! See https://doc.rust-lang.org/reference/destructors.html
    // We need the inner MutexGuard to drop before reenabling interrupts. Otherwise, there's a
    // possible deadlock where interrupts are reenabled, a pending interrupt tries to lock the
    // spinlock, and is stuck because we haven't unlocked it yet.
    // See also Linux's spin_lock_irqsave and spin_lock_irqrestore implementation:
    // https://elixir.bootlin.com/linux/v5.17.1/source/include/linux/spinlock_api_smp.h#L104
    inner: MutexGuard<'a, T>,
    _interrupt_guard: interrupts::Guard,
}

impl<T> InterruptSafeMutex<T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
        }
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<T: ?Sized> InterruptSafeMutex<T> {
    #[inline(always)]
    pub fn lock(&self) -> InterruptSafeMutexGuard<'_, T> {
        let interrupt_guard = interrupts::disable();
        InterruptSafeMutexGuard {
            _interrupt_guard: interrupt_guard,
            inner: self.inner.lock(),
        }
    }

    // TODO: is a correct try_lock implementation possible?
}

impl<'a, T: ?Sized> Deref for InterruptSafeMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

impl<'a, T: ?Sized> DerefMut for InterruptSafeMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.deref_mut()
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for InterruptSafeMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for InterruptSafeMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}
