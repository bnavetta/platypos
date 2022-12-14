//! Implementation of global and local free lists.
//!
//! The per-processor local free list is a regular singularly-linked list, used
//! as a stack. The global free list is a [Treiber stack](https://en.wikipedia.org/wiki/Treiber_stack) using a tag to avoid the ABA problem.

use core::sync::atomic::Ordering;

use hal::topology::PerProcessor;
use platypos_hal as hal;

use crate::sync::AtomicU64;
use crate::{Slot, SLOT_BITS, SLOT_MAX};

/// Concurrent, global free list.
pub(crate) struct GlobalFreeList {
    /// The top of the free stack
    head: AtomicU64,
    /// Counter incremented on every push to avoid ABA issues
    tag: AtomicU64,
}

/// Per-core free list.
pub(crate) struct LocalFreeList<TP: hal::topology::Topology> {
    free_lists: PerProcessor<usize, TP>,
}

/// Tag for if a free list is empty.
pub const EMPTY: u64 = u64::MAX;

impl GlobalFreeList {
    pub fn new(head: u64) -> Self {
        Self {
            head: AtomicU64::new(head),
            tag: AtomicU64::new(0),
        }
    }

    pub fn new_empty() -> Self {
        Self::new(EMPTY)
    }

    /// Push slot `index` onto the global free list.
    pub fn push<T>(&self, index: usize, storage: &[Slot<T>]) {
        // TODO: this should verify that `index` is not allocated and not already in a
        // free list

        debug_assert!(index < SLOT_MAX, "slot out of bounds: {index}");
        // TODO: handle tag overflow
        let new_head = (index as u64) | (self.tag.fetch_add(1, Ordering::Relaxed) << SLOT_BITS);

        loop {
            let old_head = self.head.load(Ordering::Acquire);
            storage[index].next.store(old_head, Ordering::Release);

            if self
                .head
                .compare_exchange(old_head, new_head, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Pop an element off the global free list
    pub fn pop<T>(&self, storage: &[Slot<T>]) -> Option<usize> {
        loop {
            let old_head = self.head.load(Ordering::Acquire);
            if old_head == EMPTY {
                return None;
            }

            let old_head_index = Self::to_index(old_head);
            let new_head = storage[old_head_index].next.load(Ordering::Acquire);
            if self
                .head
                .compare_exchange(old_head, new_head, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                return Some(old_head_index);
            }
        }
    }

    /// Extracts the index from a tagged stack entry
    fn to_index(entry: u64) -> usize {
        entry as usize & ((1 << SLOT_BITS) - 1)
    }
}

impl<TP: hal::topology::Topology> LocalFreeList<TP> {
    pub fn new(topology: TP) -> Self {
        Self {
            free_lists: PerProcessor::new(topology),
        }
    }

    /// Push slot `index` onto the local free list
    pub fn push<T>(&self, index: usize, storage: &[Slot<T>]) {
        // TODO: this should verify that `index` is not allocated and not already in a
        // free list
        self.free_lists.with_mut(|head| {
            let old_head = head.map(|v| v as u64).unwrap_or(EMPTY);
            storage[index].next.store(old_head, Ordering::Relaxed);
            *head = Some(index);
        });
    }

    /// Pop a slot off the local free list
    pub fn pop<T>(&self, storage: &[Slot<T>]) -> Option<usize> {
        self.free_lists.with_mut(|head| {
            let Some(old_head) = *head else { return None };
            if old_head == EMPTY as usize {
                None
            } else {
                *head = Some(storage[old_head].next.load(Ordering::Relaxed) as usize);
                Some(old_head)
            }
        })
    }
}

#[cfg(all(loom, test))]
mod test {
    use super::GlobalFreeList;
    use crate::Slot;

    use loom::sync::Arc;

    #[test]
    fn test_single_processor() {
        loom::model(|| {
            let storage: [Slot<()>; 6] = [
                Slot::empty(),
                Slot::empty(),
                Slot::empty(),
                Slot::empty(),
                Slot::empty(),
                Slot::empty(),
            ];

            let free_list = GlobalFreeList::new_empty();

            free_list.push(1, &storage);
            assert_eq!(free_list.pop(&storage), Some(1));
            assert_eq!(free_list.pop(&storage), None);

            free_list.push(4, &storage);
            free_list.push(3, &storage);
            free_list.push(5, &storage);
            assert_eq!(free_list.pop(&storage), Some(5));
            free_list.push(0, &storage);
            assert_eq!(free_list.pop(&storage), Some(0));
            assert_eq!(free_list.pop(&storage), Some(3));
            assert_eq!(free_list.pop(&storage), Some(4));
        })
    }

    #[test]
    fn test_concurrent_push() {
        loom::model(|| {
            let storage: [Slot<()>; 4] =
                [Slot::empty(), Slot::empty(), Slot::empty(), Slot::empty()];
            let storage = Arc::new(storage);
            let free_list = Arc::new(GlobalFreeList::new_empty());

            let t1 = {
                let storage = storage.clone();
                let free_list = free_list.clone();
                loom::thread::spawn(move || {
                    free_list.push(0, &*storage);
                    free_list.push(2, &*storage);
                })
            };

            let t2 = {
                let storage = storage.clone();
                let free_list = free_list.clone();
                loom::thread::spawn(move || {
                    free_list.push(1, &*storage);
                    free_list.push(3, &*storage);
                })
            };

            t1.join().unwrap();
            t2.join().unwrap();

            let mut results = vec![];
            while let Some(item) = free_list.pop(&*storage) {
                results.push(item);
            }

            results.sort();
            assert_eq!(results, vec![0, 1, 2, 3]);
        })
    }
}
