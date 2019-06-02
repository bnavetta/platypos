use alloc::boxed::Box;
use core::cell::UnsafeCell;

use hashbrown::HashMap;

pub struct CoreLocal<T> {
    init: Box<Fn() -> T>,
}
