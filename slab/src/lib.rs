//! Fixed-size lock-free concurrent slab, inspired by [sharded-slab](https://lib.rs/crates/sharded-slab).
//!
//! The main differences are:
//! - `no_std` support via HAL access to interrupt management and the current
//!   processor ID.
//! - Static, rather than dynamic, allocation, so that all operations after
//!   initialization are guaranteed not to allocate.

#![cfg_attr(not(loom), no_std)]
#![feature(maybe_uninit_array_assume_init)]

use core::mem::MaybeUninit;
use core::ops::Deref;

use modular_bitfield::specifiers::{B18, B46};
use modular_bitfield::{bitfield, Specifier};
use platypos_hal as hal;

mod free_lists;
mod slot;
mod sync;

use free_lists::{GlobalFreeList, LocalFreeList};
use slot::Slot;
use sync::ConstPtr;

pub struct Slab<
    const SIZE: usize,
    T: Sized,
    // IC: hal::interrupts::Controller,
    TP: hal::topology::Topology + 'static,
> {
    // interrupts: IC,
    topology: &'static TP,
    local_free_list: LocalFreeList<&'static TP>,
    global_free_list: GlobalFreeList,

    slots: [Slot<T>; SIZE],
}

/// Index pointing to a slab allocation
#[bitfield]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u64)]
pub struct Idx {
    generation: B46,
    index: B18,
}

/// Reference to a live slab allocation. Active references prevent slab entries
/// from being removed.
pub struct Ref<'a, const SIZE: usize, T, TP: hal::topology::Topology + 'static> {
    value: Option<ConstPtr<MaybeUninit<T>>>,
    index: Idx,
    slab: &'a Slab<SIZE, T, TP>,
}

impl<const SIZE: usize, T: Sized, TP: hal::topology::Topology + 'static> Slab<SIZE, T, TP> {
    pub fn new(topology: &'static TP) -> Self {
        assert!(
            SIZE < (1 << B18::BITS),
            "Size {SIZE} exceeds maximum slab size"
        );

        // The slab is initialized such that all slots are in the global free list, in
        // order.
        let mut slots: [MaybeUninit<Slot<T>>; SIZE] = unsafe {
            // Safety: we can call assume_init because we're claiming that a bunch of
            // MaybeUninits, which don't require initialization, are initialized.
            MaybeUninit::uninit().assume_init()
        };

        for (idx, elem) in slots.iter_mut().enumerate() {
            let next = if idx == SIZE - 1 {
                free_lists::EMPTY
            } else {
                (idx + 1) as u64
            };
            elem.write(Slot::new_unallocated(next));
        }
        // Safety: every slot was initialized in the for-loop
        let slots = unsafe { MaybeUninit::array_assume_init(slots) };
        let global_free_list = GlobalFreeList::new(0);

        Self {
            topology,
            local_free_list: LocalFreeList::new(topology),
            global_free_list,
            slots,
        }
    }

    /// Insert a new value into the slab, returning its allocated index. If
    /// there is no space left, this fails and returns the value.
    pub fn insert(&self, value: T) -> Result<Idx, T> {
        let Some(index) = self
            .local_free_list
            .pop(&self.slots)
            .or_else(|| self.global_free_list.pop(&self.slots)) else {
                return Err(value);
            };

        let slot = &self.slots[index];
        // Safety: this slot has just been allocated, but not yet returned, so no one
        // else has valid access to its contents

        let generation = unsafe { slot.allocate(value, self.topology.current_processor()) };

        Ok(Idx::new()
            .with_generation(generation)
            .with_index(index.try_into().unwrap()))
    }

    /// Removes the value at `idx` in the slab, returning `true` on success. If
    /// there are outstanding references, the value may not be immediately
    /// cleared. If the index is invalid, returns `false` instead.
    pub fn remove(&self, idx: Idx) -> bool {
        let index = idx.index() as usize;
        let Some(slot) = self.slots.get(index) else {
            return false
        };

        match slot.mark_unallocated(idx.generation()) {
            Ok(true) => {
                // There were no references, we can clear the slot
                unsafe {
                    // Safety: if mark_unallocated returns true, then the conditions for
                    // return_slot/clear are upheld
                    self.return_slot(index);
                }
                true
            }
            Ok(false) => {
                // There are outstanding references, so the slot has been marked a zombie
                true
            }
            Err(_) => false,
        }
    }

    /// Clear a slot and add it to the appropriate free list.
    ///
    /// # Safety
    /// The caller must guarantee that the slot is unallocated, per
    /// [`Slot::clear`].
    unsafe fn return_slot(&self, index: usize) {
        let slot = &self.slots[index];
        match slot.clear() {
            Some(processor) if processor == self.topology.current_processor() => {
                self.local_free_list.push(index, &self.slots);
            }
            _ => {
                self.global_free_list.push(index, &self.slots);
            }
        }
    }

    pub fn get(&self, idx: Idx) -> Option<Ref<'_, SIZE, T, TP>> {
        let slot = &self.slots.get(idx.index() as usize)?;

        let ptr = slot.acquire_reference(idx.generation()).ok()?;
        Some(Ref {
            index: idx,
            value: Some(ptr),
            slab: self,
        })
    }

    fn drop_reference(&self, idx: Idx) {
        // Use panicking array access since this is only called from Ref::drop, and all
        // Refs should have a valid index
        let slot = &self.slots[idx.index() as usize];

        let should_clear = slot
            .release_reference(idx.generation())
            .expect("slot was mutated with an active reference");
        if should_clear {
            // Safety: should_clear indicates that this was the last reference to a zombie
            // slot, so it can be returned
            unsafe { self.return_slot(idx.index() as usize) };
        }
    }
}

impl<'a, const SIZE: usize, T, TP: hal::topology::Topology + 'static> Deref
    for Ref<'a, SIZE, T, TP>
{
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // Safety: we got this ConstPtr from an allocated slot, so we know it was
        // initialized. It'd be preferable to assume_init when creating the Ref
        // in Slab, but that would break Loom's checking
        unsafe {
            self.value
                .as_ref()
                .expect("was Ref partially dropped?")
                .deref()
                .assume_init_ref()
        }
    }
}

impl<'a, const SIZE: usize, T, TP: hal::topology::Topology + 'static> Drop
    for Ref<'a, SIZE, T, TP>
{
    fn drop(&mut self) {
        self.value = None; // Ensure our reference to the contents UnsafeCell is inaccessible before
                           // calling drop_reference, which may also access the cell if removing a value
        self.slab.drop_reference(self.index)
    }
}

// could reorganize HAL to conditionally compile + depend on platform
// implementations (like rust stdlib) rather than generics everywhere
// probably better for modularity than needing type parameters for every API
// some code uses internally

// TODO: interrupt safety

#[cfg(all(test, loom))]
mod test {
    use loom::sync::Arc;

    use super::*;
    use platypos_hal::topology::loom::{LoomTopology, TOPOLOGY};

    #[test]
    fn test_migrate_entries() {
        loom::model(|| {
            let slab: Arc<Slab<16, i32, LoomTopology>> = Arc::new(Slab::new(&TOPOLOGY));

            let index = {
                let slab = slab.clone();
                loom::thread::spawn(move || slab.insert(42).unwrap())
                    .join()
                    .unwrap()
            };

            assert!(slab.remove(index));
        })
    }

    #[test]
    fn test_concurrent_insertions() {
        loom::model(|| {
            let slab: Arc<Slab<16, i32, LoomTopology>> = Arc::new(Slab::new(&TOPOLOGY));

            let t1 = {
                let slab = slab.clone();
                loom::thread::spawn(move || {
                    (0..4)
                        .map(|idx| slab.insert(idx).unwrap())
                        .collect::<Vec<_>>()
                })
            };

            let t2 = {
                let slab = slab.clone();
                loom::thread::spawn(move || {
                    (5..8)
                        .map(|idx| slab.insert(idx).unwrap())
                        .collect::<Vec<_>>()
                })
            };

            // TODO: check values as well, not just memory model
            t1.join().unwrap();
            t2.join().unwrap();
        })
    }

    #[test]
    fn test_remove_with_references() {
        loom::model(|| {
            let slab: Slab<4, i32, _> = Slab::new(&TOPOLOGY);
            let idx = slab.insert(42).unwrap();

            let reference = slab.get(idx).unwrap();
            assert_eq!(reference.deref(), &42);

            // Removed, but reference still active
            assert!(slab.remove(idx));
            assert_eq!(*reference, 42);

            drop(reference);
            assert!(slab.get(idx).is_none());
        })
    }
}
