//! Kernel synchronization primitives

use core::cell::UnsafeCell;
use core::fmt;
use core::mem::MaybeUninit;
use core::ops::{Deref, DerefMut};
use core::sync::atomic::{AtomicBool, Ordering};

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

/// Primitive for global state initialized during boot. This is similar to
/// [`spin::Once`], but optimized for the case of values that are known to be
/// initialized in a specific order, such as memory allocators and state used in
/// interrupt handlers.
///
/// # Example
///
/// ```rust
/// // In some_subsystem:
///
/// struct Driver {
///     base_address: PhysicalAddress,
/// }
///
/// pub fn init(base_address: PhysicalAddress) -> &'static Driver {
///     static GLOBAL: Global<Driver> = Global::new();
///     GLOBAL.init(Driver { base_address })
/// }
/// ```
pub struct Global<T> {
    initialized: AtomicBool,
    value: UnsafeCell<MaybeUninit<T>>,
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

impl<T> Global<T> {
    /// Create a new uninitialized `Global`
    pub const fn new() -> Self {
        Self {
            initialized: AtomicBool::new(false),
            value: UnsafeCell::new(MaybeUninit::uninit()),
        }
    }

    /// Attempt to initialize this global with `value`, returning `Err` if it
    /// has already been initialized.
    pub fn try_init(&self, value: T) -> Result<&T, ()> {
        self.initialized
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .map_err(|_| ())?;

        // SAFETY: at this point, we know `value` is uninitialized, and that any
        // other thread attempting initialization will fail because we have set
        // `initialized`
        let value_ref = unsafe { (&mut *self.value.get()).write(value) };
        Ok(value_ref)
    }

    /// Initialize this global to `value`
    ///
    /// # Panics
    /// If already initialized
    pub fn init(&self, value: T) -> &T {
        self.try_init(value).expect("global already initialized")
    }

    /// Get a reference to the value if initialized, otherwise `None`
    pub fn try_get(&self) -> Option<&T> {
        if self.initialized.load(Ordering::Acquire) {
            // SAFETY: we know that this value has been initialized from checking
            // `initialized`
            Some(unsafe { &*(*self.value.get()).as_ptr() })
        } else {
            None
        }
    }

    /// Get a reference to the value
    ///
    /// # Panics
    /// If not yet initialized
    pub fn get(&self) -> &T {
        // TODO: if I'm _really_ confident, could make the initialization check
        // a debug assertion instead of calling try_get
        self.try_get().expect("global not initialized")
    }
}

// Same unsafe impls as spin::Once
unsafe impl<T: Send + Sync> Sync for Global<T> {}
unsafe impl<T: Send> Send for Global<T> {}

impl<T> Drop for Global<T> {
    fn drop(&mut self) {
        if *self.initialized.get_mut() {
            unsafe { self.value.get_mut().assume_init_drop() };
        }
    }
}
