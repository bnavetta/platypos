use core::cell::UnsafeCell;
use core::hint::unreachable_unchecked;
use core::mem;

use arr_macro::arr;
use x86_64::instructions::interrupts::without_interrupts;

use crate::config::MAX_PROCESSORS;
use crate::topology::processor::local_id;

pub struct ProcessorLocalKey<T: 'static> {
    init: fn() -> T,

    // TODO: replace with MAX_PROCESSORS once arr! supports const variable sizes
    slots: [UnsafeCell<Option<T>>; 8],

}

impl <T: 'static> ProcessorLocalKey<T> {
    #[doc(hidden)] // Only exposed for use in macro
    pub const fn new(init: fn() -> T) -> ProcessorLocalKey<T> {
        ProcessorLocalKey {
            init,
            slots: arr![UnsafeCell::new(Option::None); 8]
        }
    }
}

impl <T: 'static> ProcessorLocalKey<T> {
    /// From LocalKey::init since there are some weird ordering requirements
    unsafe fn init(&self, slot: &UnsafeCell<Option<T>>) -> &T {
        let value = (self.init)();

        let ptr = slot.get();
        mem::replace(&mut *ptr, Some(value));

        // Get the value out. LocalKey::init explains using match instead of unwrap - basically,
        // the optimizer doesn't know to remove the panic case in unwrap
        match *ptr {
            Some(ref val) => val,
            None => unreachable_unchecked()
        }
    }

    /// Run a closure against the current processor's instance of the value. The value is lazily
    /// initialized if it has not been referenced on this processor yet.
    pub fn with<F, R>(&'static self, f: F) -> R where F: FnOnce(&T) -> R {
        debug_assert!(MAX_PROCESSORS <= 8, "ProcessorLocalKey only supports up to 8 processors");

        without_interrupts(|| {
            let key = local_id();

            let slot = &self.slots[key];
            let val = unsafe {
                match *slot.get() {
                    Some(ref val) => val,
                    None => self.init(slot)
                }
            };

            f(val)
        })
    }
}

// Using the processor ID as a key and disabling preemption inside `with` means we'll never have
// multiple threads accessing the same entry at the same time.
unsafe impl <T> Sync for ProcessorLocalKey<T> {}

// Based on the std::thread_local macro's syntax, which allows definition of multiple thread-local variables
#[macro_export]
macro_rules! processor_local {
    // Base case for recursion
    () => {};

    // Handle multiple definitions
    ($(#[$attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr; $($rest:tt)*) => (
        $crate::__processor_local_impl!($(#[$attr])* $vis $name, $t, $init);
        $crate::processor_local!($($rest)*);
    );

    // Handle a single definition
    ($(#[attr:meta])* $vis:vis static $name:ident: $t:ty = $init:expr) => (
        $crate::__processor_local_impl!($(#[$attr])* $vis $name, $t, $init);
    );
}

#[macro_export]
macro_rules! __processor_local_impl {
    ($(#[$attr:meta])* $vis:vis $name:ident, $t:ty, $init:expr) => {
        $(#[$attr])* $vis static $name: $crate::util::processor_local::ProcessorLocalKey<$t> = {
            #[inline]
            fn __init() -> $t { $init }

            $crate::util::processor_local::ProcessorLocalKey::new(__init)
        };
    };
}
