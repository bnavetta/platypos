//! Fixed-size lock-free concurrent slab, inspired by [sharded-slab](https://lib.rs/crates/sharded-slab).
//!
//! The main differences are:
//! - `no_std` support via HAL access to interrupt management and the current
//!   processor ID.
//1 - Static, rather than dynamic, allocation, so that all operations after
// initialization are guaranteed not to allocate.

use platypos_hal as hal;

pub struct Slab<
    const SIZE: usize,
    T: Sized,
    IC: hal::interrupts::Controller,
    TP: hal::topology::Topology,
> {}
