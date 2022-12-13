//! Fixed-size lock-free concurrent slab, inspired by [sharded-slab](https://lib.rs/crates/sharded-slab).
//!
//! The main differences are:
//! - `no_std` support via HAL access to interrupt management and the current
//!   processor ID.
//1 - Static, rather than dynamic, allocation, so that all operations after
// initialization are guaranteed not to allocate.

#![cfg_attr(not(loom), no_std)]

use core::mem::MaybeUninit;
use core::sync::atomic::AtomicUsize;

use platypos_hal as hal;

mod sync;

use sync::UnsafeCell;

pub struct Slab<
    const SIZE: usize,
    T: Sized,
    IC: hal::interrupts::Controller,
    TP: hal::topology::Topology,
> {
    interrupts: IC,
    topology: TP,
    global_free_list: AtomicUsize,
    slots: [Slot<T>; SIZE],
}

struct Slot<T> {
    /// The value in this slot (may not be initialized yet)
    contents: UnsafeCell<MaybeUninit<T>>,
    /// The next free list entry after this one, if this slot is in a free list
    next: UnsafeCell<usize>,
}
