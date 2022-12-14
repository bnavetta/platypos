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
use core::sync::atomic::Ordering;

use hal::topology::ProcessorId;
use platypos_hal as hal;

mod free_lists;
mod sync;

use free_lists::{GlobalFreeList, LocalFreeList};
use sync::{AtomicBool, AtomicU64, ConstPtr, UnsafeCell};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Idx(u64);

/// Reference to a live slab allocation. Active references prevent slab entries
/// from being removed.
pub struct Ref<'a, const SIZE: usize, T, TP: hal::topology::Topology + 'static> {
    value: Option<ConstPtr<MaybeUninit<T>>>,
    index: Idx,
    slab: &'a Slab<SIZE, T, TP>,
}

/// The number of bits in a slot index
pub(crate) const SLOT_BITS: usize = 17;

/// Upper bound on a slot index (`2^SLOT_BITS`)
pub(crate) const SLOT_MAX: usize = 1 << SLOT_BITS;

struct Slot<T> {
    /// State of this slot, packed for atomic updates
    ///
    /// Layout (64 bits):
    /// GGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGGLRRRRRRRRRRRRRRRR
    ///
    /// G (generation): bits 0-46
    ///                 generation counter to avoid ABA problem. This is the
    ///                 same number of generation its as in a slot index,
    ///                 optimizing for long usage (many generations) of a
    ///                 smallish (2^17) number of entries
    ///
    /// L (liveness):   bit 47
    ///                 1 if the slot is allocated, 0 if it is
    ///                 unallocated (or being unallocated)
    ///
    /// R (refcount):   bits 48-63
    ///                 number of active references to this slot, only if
    ///                 allocated. Bounded by MAX_CORES.
    state: AtomicU64,

    /// Should this slot be removed once all outstanding references are cleared?
    should_remove: AtomicBool,

    /// The value in this slot (may not be initialized, depending on state)
    contents: UnsafeCell<MaybeUninit<T>>,
    /// The next free list entry after this one, if this slot is in a free list
    next: AtomicU64,

    /// The processor this slot was allocated on
    home: UnsafeCell<Option<ProcessorId>>,
}

/// Errors when updating a slot's state
#[derive(Debug, Clone, Copy)]
enum StateError {
    /// The slot was in an invalid state (for example, it had the wrong
    /// generation number)
    Invalid,
    /// The slot cannot be modified (for example, it has a nonzero reference
    /// count)
    Busy,
}

impl<const SIZE: usize, T: Sized, TP: hal::topology::Topology + 'static> Slab<SIZE, T, TP> {
    pub fn new(topology: &'static TP) -> Self {
        // The slab is initialized such that all slots are in the global free list, in
        // order.

        let mut slots: [MaybeUninit<Slot<T>>; SIZE] = unsafe {
            // Safety: we can call assume_init because we're claiming that a bunch of
            // MaybeUninits, which don't require initialization, are initialized.
            MaybeUninit::uninit().assume_init()
        };

        for (idx, elem) in slots.iter_mut().enumerate() {
            elem.write(Slot::new((idx + 1) as u64));
        }
        // Safety: every slot was initialized in the for-loop
        let slots = unsafe { MaybeUninit::array_assume_init(slots) };
        // Fix up the last slot so that it marks the end of the free list, rather than
        // pointing to a nonexistent slot.
        slots[SIZE - 1]
            .next
            .store(free_lists::EMPTY, Ordering::Relaxed);
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
        let generation = slot.generation() + 1;
        slot.set_state(generation, true, 0);
        slot.home.with_mut(|home| unsafe {
            // Safety: this slot has just been allocated, but not yet returned, so no one
            // else has valid access to its contents
            *home = Some(self.topology.current_processor())
        });
        slot.contents.with_mut(|contents| unsafe {
            // Safety: we have just allocated this slot, so it's ok to write to it
            contents.as_mut().unwrap().write(value);
        });

        Ok(Idx::new(generation, index))
    }

    /// Removes the value at `idx` in the slab, returning `true` on success. If
    /// there are outstanding references, the value may not be immediately
    /// cleared. If the index is invalid, returns `false` instead.
    pub fn remove(&self, idx: Idx) -> bool {
        let index = idx.slot();
        match self.slots[index].set_unallocated(idx.generation()) {
            Ok(()) => {
                self.slots[index].contents.with_mut(|ptr| {
                    // Safety: we were able to mark this slot as unallocated, so there are no other
                    // witnesses
                    unsafe {
                        let contents = ptr.as_mut().unwrap();
                        contents.assume_init_drop();
                    }
                });

                // Safety: we successfully marked this slot as unallocated, but have not yet
                // added it to a free list. So:
                // - there were no live references to it
                // - no other processors will attempt to allocate it
                let home = self.slots[index]
                    .home
                    .with(|h| unsafe { *h }.expect("previously-allocated slot had no home"));

                // Following sharded-slab and mimalloc, if the slot is freed on the same
                // processor it was allocated on, add it back to the local free list to reduce
                // contention. Otherwise, return it to the global free list. Over time,
                // processors will collect slots proportionally to how many they use.
                if home == self.topology.current_processor() {
                    self.local_free_list.push(index, &self.slots);
                } else {
                    self.global_free_list.push(index, &self.slots);
                }

                true
            }
            Err(StateError::Busy) => {
                // TODO: I think there's a TOCTTOU issue here if the last reference disappears
                //       between calling set_unallocated and setting should_remove
                self.slots[index]
                    .should_remove
                    .store(true, Ordering::Release);
                true
            }
            Err(StateError::Invalid) => false,
        }
    }

    pub fn get(&self, idx: Idx) -> Option<Ref<'_, SIZE, T, TP>> {
        let slot = &self.slots.get(idx.slot())?;
        match slot.mutate_refcount(idx.generation(), |r| r + 1) {
            Ok(_) => {
                let reference = slot.contents.get();
                Some(Ref {
                    index: idx,
                    value: Some(reference),
                    slab: self,
                })
            }
            Err(_) => None,
        }
    }

    fn drop_reference(&self, idx: Idx) {
        let slot = &self.slots[idx.slot()];
        let prev_refcount = slot
            .mutate_refcount(idx.generation(), |r| r - 1)
            .expect("slot was mutated with an active reference");

        // TODO: need to make "pending removal" a _state_
        if slot.should_remove.load(Ordering::Acquire) && prev_refcount == 1 {
            // Removal was requested and this was the last reference, so delete
            // it now
            self.remove(idx);
        }
    }
}

impl Idx {
    // Index representation:
    // - bits 0-46 are the generation (same as Slot::state)
    // - bits 47-63 are the slot number
    // This means a slab can hold up to 2^17 = 131072 live entries

    fn new(generation: u64, slot: usize) -> Self {
        debug_assert!(
            generation < (1 << 48),
            "generation out of bounds: {generation}"
        );
        debug_assert!(slot < SLOT_MAX, "slot out of bounds: {slot}");
        Self(generation | (slot as u64) << 46)
    }

    /// Convert this index to a 64-bit value. The exact representation should
    /// not be relied on and may change without notice.
    pub fn as_u64(self) -> u64 {
        self.0
    }

    /// The generation number for this index, used to handle the ABA problem.
    fn generation(self) -> u64 {
        self.0 & 0x3fffffffffff
    }

    /// The slot number that this index points to
    fn slot(self) -> usize {
        (self.0 >> 46) as usize
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

impl<T> Slot<T> {
    fn empty() -> Self {
        Self::new(crate::free_lists::EMPTY)
    }

    /// Initialize a new unallocated `Slot`, with the given `next` pointer,
    /// generation 0, and no contents.
    ///
    /// This is only useful when initializing a `Slab`, where the free list must
    /// also be initialized.
    fn new(next: u64) -> Self {
        Self {
            state: AtomicU64::new(Self::pack_state(0, false, 0)),
            contents: UnsafeCell::new(MaybeUninit::uninit()),
            next: AtomicU64::new(next),
            should_remove: AtomicBool::new(false),
            home: UnsafeCell::new(None),
        }
    }

    /// Atomically overwrite this slot's state. This uses [`Ordering::Relaxed`]
    /// and does not verify the previous state, and so it must only be used if
    /// the slot is already protected.
    fn set_state(&self, generation: u64, allocated: bool, refcount: u16) {
        self.state.store(
            Self::pack_state(generation, allocated, refcount),
            Ordering::Relaxed,
        );
    }

    /// Read the slot's current generation. This uses [`Ordering::Relaxed`] so
    /// it must only be used if the slot is already protected.
    fn generation(&self) -> u64 {
        let (generation, _, _) = Self::unpack_state(self.state.load(Ordering::Relaxed));
        generation
    }

    fn set_unallocated(&self, expected_generation: u64) -> Result<(), StateError> {
        let old_state = Self::pack_state(expected_generation, true, 0);
        let new_state = Self::pack_state(expected_generation, false, 0);

        match self
            .state
            .compare_exchange(old_state, new_state, Ordering::AcqRel, Ordering::Acquire)
        {
            Ok(_) => Ok(()),
            Err(state) => {
                let (_, _, refcount) = Self::unpack_state(state);
                if refcount == 0 {
                    // either the generation was incorrect or the slot was already unallocated
                    Err(StateError::Invalid)
                } else {
                    // there are live references to the slot
                    Err(StateError::Busy)
                }
            }
        }
    }

    /// Applies `f` to modify the reference count, returning the previous value
    fn mutate_refcount(
        &self,
        expected_generation: u64,
        mut f: impl FnMut(u16) -> u16,
    ) -> Result<u16, StateError> {
        match self
            .state
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |state| {
                let (generation, allocated, refcount) = Self::unpack_state(state);
                if generation != expected_generation || !allocated {
                    None
                } else {
                    Some(Self::pack_state(generation, allocated, f(refcount)))
                }
            }) {
            Ok(prev) => {
                let (_, _, refcount) = Self::unpack_state(prev);
                Ok(refcount)
            }
            Err(_) => Err(StateError::Invalid),
        }
    }

    // use fetch_update to adjust reference count

    /// Packs a generation, allocation flag, and reference count into a 64-bit
    /// state value.
    ///
    /// # Panics
    /// In debug mode, if the generation is too large.
    #[inline(always)]
    fn pack_state(generation: u64, allocated: bool, refcount: u16) -> u64 {
        debug_assert!(
            generation < (1 << 48),
            "generation out of bounds: {generation}"
        );
        let mut state = generation;
        if allocated {
            state |= 1 << 47;
        }
        state |= (refcount as u64) << 48;

        state
    }

    /// Unpacks a 64-bit state value into the generation, allocation flag, and
    /// reference count.
    #[inline(always)]
    fn unpack_state(state: u64) -> (u64, bool, u16) {
        let generation = state & 0x3fffffffffff;
        let allocated = state & (1 << 47) != 0;
        let refcount = (state >> 48) as u16;
        (generation, allocated, refcount)
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
