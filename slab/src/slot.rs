//! Slots are individual entries in the slab.
//! Each slot contains:
//! - a lifecycle field

use core::mem::MaybeUninit;
use core::sync::atomic::Ordering;

use modular_bitfield::specifiers::{B16, B46};
use modular_bitfield::{bitfield, BitfieldSpecifier};
use platypos_hal::topology::ProcessorId;

use crate::sync::{AtomicU64, ConstPtr, UnsafeCell};

#[derive(BitfieldSpecifier)]
#[bits = 2]
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum State {
    /// Marks a slot that is allocated
    Allocated,
    /// Marks a slot that is unallocated
    Unallocated,
    /// Marks a slot that has been removed, but has outstanding references. It
    /// will be unallocated once the last reference is dropped.
    Zombie,
}

/// Lifecycle status of a slot
#[bitfield]
#[derive(Debug)]
#[repr(u64)]
#[allow(dead_code)]
pub(crate) struct Lifecycle {
    /// The state of the slot (allocated/unallocated)
    state: State,
    /// The current generation number of this slot. It's incremented on every
    /// insertion to avoid the ABA problem.
    generation: B46,
    /// Number of active references to this slot, if it's allocated.
    refcount: B16,
}

pub(crate) struct Slot<T> {
    /// Lifecycle of the slot. This is a packed [`Lifecycle`] value.
    lifecycle: AtomicU64,
    /// Contents of the slot, only initialized if it is allocated or a zombie.
    contents: UnsafeCell<MaybeUninit<T>>,
    /// The next free list entry after this one, if it is in a free list.
    next: UnsafeCell<u64>,
    /// The processor this slot was most recently allocated on
    processor: UnsafeCell<Option<ProcessorId>>,
}

/// Error indicating that a slot's lifecycle was not as expected
#[derive(Debug)]
pub(crate) enum LifecycleError {
    /// The generations did not match. This occurs
    /// when using old/invalid indices.
    WrongGeneration {
        expected_generation: u64,
        actual_generation: u64,
    },
    /// The state was unexpected. For example, a slot may have been unallocated
    /// when it was expected to be allocated.
    WrongState {
        expected_state: State,
        actual_state: State,
    },
}

impl<T> Slot<T> {
    /// A new, unallocated slot. This slot will be in the [`State::Unallocated`]
    /// state, with its free list pointer set to `next`.
    pub(crate) fn new_unallocated(next: u64) -> Self {
        Self {
            lifecycle: AtomicU64::new(
                Lifecycle::new()
                    .with_generation(0)
                    .with_state(State::Unallocated)
                    .with_refcount(0)
                    .into(),
            ),
            contents: UnsafeCell::new(MaybeUninit::uninit()),
            next: UnsafeCell::new(next),
            processor: UnsafeCell::new(None),
        }
    }

    /// Initialize this slot as allocated.
    ///
    /// # Safety
    /// The caller must guarantee that it is allowed to allocate this slot, and
    /// that it was previously unallocated. In general, it should have just
    /// been removed from a free list.
    pub(crate) unsafe fn allocate(&self, value: T, current_processor: ProcessorId) -> u64 {
        // Relaxed ordering is OK here because this is an unallocated,
        // not-on-the-free-list slot, so no other cores should access it
        let mut lifecycle = Lifecycle::from(self.lifecycle.load(Ordering::Relaxed));
        let next_generation = lifecycle
            .generation()
            .checked_add(1)
            .expect("generation overflow");
        lifecycle.set_generation(next_generation);
        lifecycle.set_state(State::Allocated);
        debug_assert!(lifecycle.refcount() == 0, "unallocated slot had references");
        self.lifecycle.store(lifecycle.into(), Ordering::Relaxed);

        // Safety: this slot has been allocated but not yet returned, so no
        // other cores have access to it
        self.contents.with_mut(|ptr| {
            ptr.as_mut().unwrap().write(value);
        });
        self.processor
            .with_mut(|ptr| *ptr = Some(current_processor));
        // self.next.with_mut(|ptr| *ptr = crate::free_lists::EMPTY); // Don't reset
        // next because that races with the free list checking it spuriously (spurious
        // because if the list is modified, we won't store it as the new head and will
        // instead fetch again)
        next_generation
    }

    /// Acquire a reference to this slot by bumping its reference count.
    pub(crate) fn acquire_reference(
        &self,
        expected_generation: u64,
    ) -> Result<ConstPtr<MaybeUninit<T>>, LifecycleError> {
        let mut prev_lifecycle = self.lifecycle.load(Ordering::Acquire);
        loop {
            let mut lifecycle = Lifecycle::from(prev_lifecycle);
            if lifecycle.generation() != expected_generation {
                return Err(LifecycleError::WrongGeneration {
                    expected_generation,
                    actual_generation: lifecycle.generation(),
                });
            }
            if lifecycle.state() != State::Allocated {
                return Err(LifecycleError::WrongState {
                    expected_state: State::Allocated,
                    actual_state: lifecycle.state(),
                });
            }

            lifecycle.set_refcount(
                lifecycle
                    .refcount()
                    .checked_add(1)
                    .expect("refcount overflow"),
            );
            match self.lifecycle.compare_exchange(
                prev_lifecycle,
                lifecycle.into(),
                Ordering::Release,
                Ordering::Acquire,
            ) {
                // TODO: we know the contents are initialized at this point, so it'd be cleaner to
                // return a T pointer rather than MaybeUninit. However, we have to keep the ConstPtr
                // returned by .get() so that Loom can track concurrent access
                Ok(_) => return Ok(self.contents.get()),
                Err(actual) => prev_lifecycle = actual,
            }
        }
    }

    /// Release a reference to this slot. Returns `true` if this was the last
    /// reference to a zombie slot, in which case the caller must clear it.
    pub(crate) fn release_reference(
        &self,
        expected_generation: u64,
    ) -> Result<bool, LifecycleError> {
        let mut prev_lifecycle = self.lifecycle.load(Ordering::Acquire);
        loop {
            let mut lifecycle = Lifecycle::from(prev_lifecycle);
            if lifecycle.generation() != expected_generation {
                return Err(LifecycleError::WrongGeneration {
                    expected_generation,
                    actual_generation: lifecycle.generation(),
                });
            }
            if lifecycle.state() == State::Unallocated {
                return Err(LifecycleError::WrongState {
                    expected_state: State::Allocated,
                    actual_state: State::Unallocated,
                });
            }

            lifecycle.set_refcount(
                lifecycle
                    .refcount()
                    .checked_sub(1)
                    .expect("released a reference but refcount was 0"),
            );
            // Was this the last reference to a zombie? If so, the caller should clear it
            // once we successfully update the lifecycle
            let should_clear = lifecycle.state() == State::Zombie && lifecycle.refcount() == 0;

            match self.lifecycle.compare_exchange(
                prev_lifecycle,
                lifecycle.into(),
                Ordering::Release,
                Ordering::Acquire,
            ) {
                Ok(_) => return Ok(should_clear),
                Err(actual) => prev_lifecycle = actual,
            }
        }
    }

    /// Mark this slot as unallocated. If there are outstanding references, it
    /// will instead be marked as a zombie.
    ///
    /// # Returns
    /// `true` if the slot was unallocated, `false` if it became a zombie
    ///
    /// # Errors
    /// If the slot's generation does not match `expected_generation`
    pub(crate) fn mark_unallocated(
        &self,
        expected_generation: u64,
    ) -> Result<bool, LifecycleError> {
        // // First, optimistically assume there are no references and try to
        // modify the // state accordingly
        // let assumed_lifecycle = Lifecycle::new()
        //     .with_generation(expected_generation)
        //     .with_state(State::Allocated)
        //     .with_refcount(0);
        // let desired_lifecycle = Lifecycle::new()
        //     .with_generation(expected_generation)
        //     .with_state(State::Unallocated)
        //     .with_refcount(0);
        // let mut refcount = match self.lifecycle.compare_exchange(
        //     assumed_lifecycle.into(),
        //     desired_lifecycle.into(),
        //     Ordering::Acquire,
        //     Ordering::Acquire,
        // ) {
        //     // Success! The slot was allocated with 0 references
        //     Ok(_) => return Ok(true),
        //     Err(actual) => {
        //         // Failure - figure out why
        //         let actual_lifecycle = Lifecycle::from(actual);
        //         if actual_lifecycle.generation() != expected_generation {
        //             return Err(LifecycleError::WrongGeneration {
        //                 expected_generation,
        //                 actual_generation: actual_lifecycle.generation(),
        //             });
        //         } else if actual_lifecycle.state() != State::Allocated {
        //             return Err(LifecycleError::WrongState {
        //                 expected_state: State::Allocated,
        //                 actual_state: actual_lifecycle.state(),
        //             });
        //         } else {
        //             assert!(
        //                 actual_lifecycle.refcount() > 0,
        //                 "compare-and-exchange failed but lifecycle matches
        // expected value"             );
        //             actual_lifecycle.refcount()
        //         }
        //     }
        // };

        // If we're here, there are active references, so loop making a zombie
        // (in case refcount changes) TODO: references could disappear
        // in the meantime, switch back to fetch_update? or loop the whole
        // thing?

        let mut was_unallocated = None;
        let outcome =
            self.lifecycle
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |lifecycle| {
                    let mut lifecycle = Lifecycle::from(lifecycle);

                    if lifecycle.generation() != expected_generation
                        || lifecycle.state() != State::Allocated
                    {
                        // The slot was in an invalid state, so we can't mark it
                        None
                    } else if lifecycle.refcount() > 0 {
                        // There are outstanding references, so mark the slot as
                        // a zombie
                        lifecycle.set_state(State::Zombie);
                        was_unallocated = Some(false);
                        Some(lifecycle.into())
                    } else {
                        // There are no outstanding references, so mark the slot as unallocated
                        lifecycle.set_state(State::Unallocated);
                        was_unallocated = Some(true);
                        Some(lifecycle.into())
                    }
                });

        match outcome {
            // If fetch_update returns Ok, then it applied the update to the stored value, and so
            // was_unallocated must have been set
            Ok(_) => Ok(was_unallocated.unwrap()),
            Err(lifecycle) => {
                let lifecycle = Lifecycle::from(lifecycle);
                if lifecycle.generation() != expected_generation {
                    Err(LifecycleError::WrongGeneration {
                        expected_generation,
                        actual_generation: lifecycle.generation(),
                    })
                } else {
                    Err(LifecycleError::WrongState {
                        expected_state: State::Allocated,
                        actual_state: lifecycle.state(),
                    })
                }
            }
        }
    }

    /// Clears out this slot by:
    /// 1. Marking it as unallocated
    /// 2. Dropping its contents
    ///
    /// # Safety
    /// The caller must be allowed to clear the slot. Either a
    /// [`mark_unallocated`] call returned `Ok(true)` or the caller has
    /// just removed the last reference to a zombie slot.
    ///
    /// If these conditions are not met, existing data will be overwritten.
    pub(crate) unsafe fn clear(&self) -> Option<ProcessorId> {
        // Relaxed ordering is fine since the slot is being cleared - no other cores are
        // using it
        let mut lifecycle = Lifecycle::from(self.lifecycle.load(Ordering::Relaxed));
        debug_assert_eq!(lifecycle.refcount(), 0, "clearing a slot with references");
        lifecycle.set_state(State::Unallocated);
        self.lifecycle.store(lifecycle.into(), Ordering::Relaxed);

        self.contents.with_mut(|ptr| unsafe {
            // Safety: the slot was previously allocated, so we know it has contents to drop
            ptr.as_mut()
                .expect("slot contents pointer is null")
                .assume_init_drop();
        });
        // Safety: as above, no one else can access this slot, so we can clear out the
        // processor field
        self.processor.with_mut(|ptr| unsafe {
            let prev = *ptr;
            *ptr = None;
            prev
        })
    }

    /// Set the `next` pointer of this slot
    ///
    /// # Safety
    /// The caller must ensure that the slot is unallocated and not accessed by
    /// any other cores. In general, that means it should only be called when
    /// adding a slot to the free list.
    pub(crate) unsafe fn set_next(&self, next: u64) {
        self.next.with_mut(|ptr| *ptr = next);
    }

    /// Get the `next` pointer of this slot
    ///
    /// # Safety
    /// The caller must ensure that no other cores are mutating the slot
    pub(crate) unsafe fn next(&self) -> u64 {
        self.next.with(|ptr| *ptr)
    }
}
