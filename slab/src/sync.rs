#[cfg(loom)]
pub(crate) use loom::cell::UnsafeCell;

#[cfg(loom)]
pub(crate) use loom::cell::ConstPtr;

#[cfg(loom)]
pub(crate) use loom::sync::atomic::AtomicU64;

#[cfg(not(loom))]
pub(crate) struct UnsafeCell<T>(core::cell::UnsafeCell<T>);

#[cfg(not(loom))]
pub(crate) struct ConstPtr<T>(*const T);

#[cfg(not(loom))]
pub(crate) use core::sync::atomic::AtomicU64;

#[cfg(not(loom))]
impl<T> UnsafeCell<T> {
    pub(crate) fn new(data: T) -> UnsafeCell<T> {
        UnsafeCell(core::cell::UnsafeCell::new(data))
    }

    pub(crate) fn with<R>(&self, f: impl FnOnce(*const T) -> R) -> R {
        f(self.0.get())
    }

    pub(crate) fn with_mut<R>(&self, f: impl FnOnce(*mut T) -> R) -> R {
        f(self.0.get())
    }

    pub(crate) fn get(&self) -> ConstPtr<T> {
        ConstPtr(self.0.get())
    }
}

#[cfg(not(loom))]
impl<T> ConstPtr<T> {
    pub(crate) unsafe fn deref(&self) -> &T {
        self.0.as_ref().expect("UnsafeCell pointer is null")
    }
}
