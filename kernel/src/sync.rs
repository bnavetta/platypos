//! Kernel synchronization primitives

use core::fmt;
use core::marker::PhantomData;
use core::ops::{Deref, DerefMut};

use platypos_platform::{Platform, Processor};
use spin::{Mutex, MutexGuard};

pub struct InterruptSafeMutex<P: Platform, T: ?Sized> {
    phantom: PhantomData<P>,
    inner: Mutex<T>,
}

pub struct InterruptSafeMutexGuard<'a, P: Platform, T: ?Sized> {
    // The order of these fields is important! See https://doc.rust-lang.org/reference/destructors.html
    // We need the inner MutexGuard to drop before reenabling interrupts. Otherwise, there's a
    // possible deadlock where interrupts are reenabled, a pending interrupt tries to lock the
    // spinlock, and is stuck because we haven't unlocked it yet.
    // See also Linux's spin_lock_irqsave and spin_lock_irqrestore implementation:
    // https://elixir.bootlin.com/linux/v5.17.1/source/include/linux/spinlock_api_smp.h#L104
    inner: MutexGuard<'a, T>,
    _interrupt_guard: <<P as Platform>::Processor as Processor>::InterruptGuard,
}

impl<P: Platform, T> InterruptSafeMutex<P, T> {
    pub const fn new(value: T) -> Self {
        Self {
            inner: Mutex::new(value),
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    #[allow(dead_code)]
    pub fn into_inner(self) -> T {
        self.inner.into_inner()
    }
}

impl<P: Platform, T: ?Sized> InterruptSafeMutex<P, T> {
    #[inline(always)]
    pub fn lock(&self) -> InterruptSafeMutexGuard<'_, P, T> {
        let interrupt_guard = P::Processor::disable_interrupts();
        InterruptSafeMutexGuard {
            _interrupt_guard: interrupt_guard,
            inner: self.inner.lock(),
        }
    }

    // TODO: is a correct try_lock implementation possible?
}

impl<'a, P: Platform, T: ?Sized> Deref for InterruptSafeMutexGuard<'a, P, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.inner.deref()
    }
}

impl<'a, P: Platform, T: ?Sized> DerefMut for InterruptSafeMutexGuard<'a, P, T> {
    fn deref_mut(&mut self) -> &mut T {
        self.inner.deref_mut()
    }
}

impl<'a, P: Platform, T: ?Sized + fmt::Debug> fmt::Debug for InterruptSafeMutexGuard<'a, P, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&self.inner, f)
    }
}

impl<'a, P: Platform, T: ?Sized + fmt::Display> fmt::Display for InterruptSafeMutexGuard<'a, P, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Display::fmt(&self.inner, f)
    }
}
