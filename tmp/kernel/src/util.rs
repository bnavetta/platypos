//! General-purpose utility code used throughout the kernel.

use core::hash::{BuildHasher, Hasher, Hash};
use core::mem::{MaybeUninit, replace};

use ahash::RandomState;

/// A HashMap that holds a fixed number of entries.
///
/// This implementation uses [open addressing](https://en.wikipedia.org/wiki/Open_addressing) with linear probing.
pub struct BoundedHashMap<K, V, S, const CAPACITY: usize> {
    hasher: S,
    data: [Option<Entry<K, V>>; CAPACITY]
}

struct Entry<K, V> {
    key: K,
    value: V
}

impl <K: Hash + Eq, V, S: BuildHasher, const CAPACITY: usize> BoundedHashMap<K, V, S, CAPACITY> {
    /// Creates a new BoundedHashMap using the given hashing algorithm.
    pub fn with_hasher(hasher: S) -> BoundedHashMap<K, V, S, CAPACITY> {
        // This use of MaybeUninit is necessary (as opposed to [None; Capacity]) because Option<Entry<K, V>> is not necessarily Copy
        let mut data: [MaybeUninit<Option<Entry<K, V>>>; CAPACITY] = MaybeUninit::uninit_array();
        for entry in data.iter_mut() {
            *entry = MaybeUninit::new(None);
        }
        BoundedHashMap {
            hasher,
            // Safety: we initialized every element of the array to None above
            data: unsafe { MaybeUninit::array_assume_init(data) }
            // data: Default::default(),
        }
    }

    /// Looks up a key in the hash map.
    #[inline]
    pub fn get(&self, key: &K) -> Option<&V> {
        self.find_slot(key).and_then(|slot| self.data[slot].as_ref().map(|entry| {
            debug_assert!(&entry.key == key, "find_slot returned the wrong slot!");
            &entry.value
        }))
    }

    /// Looks up a key in the hash map, returning a mutable reference
    #[inline]
    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        let slot = self.find_slot(key)?;
        let entry = self.data[slot].as_mut()?;
        debug_assert!(&entry.key == key, "find_slot returned the wrong slot!");
        Some(&mut entry.value)
    }

    /// Insert a new entry into the hash map.
    /// If the map is full, `Err(value)` is returned.
    /// Otherwise, if the map already contained an entry for `key`, it's updated with the new value and
    /// `Ok(<previous value>)` is returned. If the map did not contain `key`, then a new entry is created
    /// and `Ok(None)` is returned.
    #[inline]
    pub fn insert(&mut self, key: K, value: V) -> Result<Option<V>, V> {
        match self.find_slot(&key) {
            Some(index) => match &mut self.data[index] {
                Some(entry) => Ok(Some(entry.set(value))),
                None => {
                    self.data[index] = Some(Entry::new(key, value));
                    Ok(None)
                }
            },
            None => Err(value)
        }
    }

    /// Removes an entry from the map, returning whether or not it existed
    #[inline]
    pub fn remove(&mut self, key: &K) -> bool {
        match self.find_slot(key) {
            Some(index) => {
                let found = self.data[index].is_some();
                self.data[index] = None;
                found
            },
            None => false,
        }
    }

    /// Finds the backing array slot for a given key. This is the first index starting at `hash(key) % CAPACITY` that
    /// is either empty or contains `key`.
    /// * If the table contains `key`, the returned index points to a non-empty slot for it
    /// * If the table does not contain `key`, but is not full, the returned index points to an empty slot where it could be inserted
    /// * If the table is full and does not contain `key`, this returns None
    /// The worst-case scenario is looking for a key that isn't in the table when the map is full. This requires iterating through every slot.
    #[inline(always)]
    fn find_slot(&self, key: &K) -> Option<usize> {
        let mut hasher = self.hasher.build_hasher();
        key.hash(&mut hasher);
        let initial_index = hasher.finish() as usize % CAPACITY;
        
        // First, scan after the hash-based index until we find the key or an empty slot (which means the key isn't there)
        for index in initial_index..CAPACITY {
            match &self.data[index] {
                None => return Some(index),
                Some(entry) if &entry.key == key => return Some(index),
                _ => ()
            }
        }

        // Now, repeat the scan from the start of the table (in case we previously wrapped around). Putting this in a second loop makes it easier to not
        // accidentally iterate forever.
        for index in 0..initial_index {
            match &self.data[index] {
                None => return Some(index),
                Some(entry) if &entry.key == key => return Some(index),
                _ => ()
            }
        }

        None
    }
}

impl <K: Hash + Eq, V, const CAPACITY: usize> BoundedHashMap<K, V, RandomState, CAPACITY> {
    pub fn new() -> BoundedHashMap<K, V, RandomState, CAPACITY> {
        BoundedHashMap::with_hasher(RandomState::new())
    }
}

impl <K, V> Entry<K, V> {
    fn new(key: K, value: V) -> Entry<K, V> {
        Entry { key, value }
    }

    /// Updates this entry with a new value, returning the previous one.
    fn set(&mut self, new_value: V) -> V {
        replace(&mut self.value, new_value)
    }
}
