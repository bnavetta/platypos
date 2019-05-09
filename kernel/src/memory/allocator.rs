use core::alloc::{GlobalAlloc, Layout};
use core::cmp::max;
use core::mem;
use core::ptr;

use array_init::array_init;
use bit_field::BitField;
use intrusive_collections::{intrusive_adapter, KeyAdapter, RBTree, RBTreeLink, UnsafeRef};

const ALIGNMENT: usize = 8;

const fn align(value: usize) -> usize {
    let mask = ALIGNMENT - 1;
    (value | mask) + 1
}

const fn is_aligned(value: usize) -> bool {
    value & (ALIGNMENT - 1) == 0
}

#[repr(C)] // So we know field order matches definition
pub struct Block {
    tag: usize,
    free_link: RBTreeLink, // ONLY VALID IF FREE
}

impl Block {
    const TAG_SIZE: usize = mem::size_of::<usize>(); // The size of a header/footer tag
    const MIN_SIZE: usize = align(mem::size_of::<Block>() + Block::TAG_SIZE); // The smallest allowed block size

    unsafe fn from_payload(payload: *mut u8) -> *mut Block {
        payload.offset(-(Block::TAG_SIZE as isize)) as *mut Block
    }

    /// Returns whether or not this block is allocated
    fn is_allocated(&self) -> bool {
        self.tag.get_bit(0)
    }

    /// Returns the size of this block
    fn size(&self) -> usize {
        self.tag & !1
    }

    fn end_tag(&self) -> *const usize {
        let ptr = (self as *const Block) as *const u8;
        let tag_ptr = unsafe { ptr.add(self.size() - Block::TAG_SIZE) };
        tag_ptr as *const usize
    }

    fn end_tag_mut(&mut self) -> *mut usize {
        let ptr = (self as *mut Block) as *mut u8;
        let tag_ptr = unsafe { ptr.add(self.size() - Block::TAG_SIZE) };
        tag_ptr as *mut usize
    }

    /// Returns whether or not the start and end tags match
    fn tags_match(&self) -> bool {
        self.tag == unsafe { self.end_tag().read() }
    }

    /// Set the size of this block. The size must be aligned to ALIGNMENT.
    fn set_size(&mut self, size: usize) {
        debug_assert!(
            is_aligned(size),
            "Size {} is not aligned to {}",
            size,
            ALIGNMENT
        );
        debug_assert!(
            size >= Block::MIN_SIZE,
            "Size {} is less than minimum block size {}",
            size,
            Block::MIN_SIZE
        );

        let mut tag = size;
        tag.set_bit(0, self.is_allocated());

        self.tag = tag;
        unsafe {
            self.end_tag_mut().write(tag);
        }
    }

    /// Set this block's allocated flag
    fn set_allocated(&mut self, allocated: bool) {
        self.tag.set_bit(0, allocated);
        unsafe { self.end_tag_mut().write(self.tag) }
    }

    /// Get a reference to the next block in memory
    fn next(&self) -> &Block {
        let ptr = (self as *const Block) as *const u8;
        unsafe {
            let next_ptr = ptr.add(self.size());
            (next_ptr as *const Block).as_ref().unwrap()
        }
    }

    /// Get a mutable reference to the next block in memory
    fn next_mut(&self) -> &mut Block {
        let ptr = (self as *const Block) as *const u8;
        unsafe {
            let next_ptr = ptr.add(self.size());
            (next_ptr as *mut Block).as_mut().unwrap()
        }
    }

    /// Get a reference to the previous block in memory
    fn prev(&self) -> &Block {
        let ptr = (self as *const Block) as *const usize;
        unsafe {
            let prev_end_tag = ptr.offset(-1).read();
            let prev_size = prev_end_tag & !1;
            let next_ptr = (ptr as *const u8).sub(prev_size);
            (next_ptr as *const Block).as_ref().unwrap()
        }
    }

    /// Get a mutable reference to the previous block in memory
    fn prev_mut(&self) -> &mut Block {
        let ptr = (self as *const Block) as *const usize;
        unsafe {
            let prev_end_tag = ptr.offset(-1).read();
            let prev_size = prev_end_tag & !1;
            let next_ptr = (ptr as *const u8).sub(prev_size);
            (next_ptr as *mut Block).as_mut().unwrap()
        }
    }
}

intrusive_adapter!(FreeBlockAdapter = UnsafeRef<Block> : Block { free_link: RBTreeLink });

// Key blocks by their memory address
impl<'a> KeyAdapter<'a> for FreeBlockAdapter {
    type Key = usize;

    fn get_key(&self, value: &'a Block) -> usize {
        (value as *const Block) as usize
    }
}

pub struct MemoryAllocator {
    fixed_free_lists: [RBTree<FreeBlockAdapter>; MemoryAllocator::FIXED_FREE_LISTS],
    approx_free_lists: [RBTree<FreeBlockAdapter>; MemoryAllocator::APPROX_FREE_LISTS],
    large_free_list: RBTree<FreeBlockAdapter>,
}

impl MemoryAllocator {
    const MAX_FIXED_SIZE: usize = 512; // largest size that goes in a fixed free list
    const FIXED_FREE_LISTS: usize = MemoryAllocator::MAX_FIXED_SIZE / ALIGNMENT;
    const APPROX_FREE_LISTS: usize = 4;

    pub fn new() -> MemoryAllocator {
        // TODO: arr! macro seemed nicer, but procedural macros don't seem to work with cargo-xbuild
        MemoryAllocator {
            //            fixed_free_lists: arr![RBTree::new(FreeBlockAdapter::new()); KernelAllocator::FIXED_FREE_LISTS],
            fixed_free_lists: array_init(|_| RBTree::new(FreeBlockAdapter::new())),
            approx_free_lists: array_init(|_| RBTree::new(FreeBlockAdapter::new())),
            large_free_list: RBTree::new(FreeBlockAdapter::new()),
        }
    }

    /// Returns the free list which would contain blocks of the given size
    fn free_list_containing(&self, size: usize) -> &RBTree<FreeBlockAdapter> {
        debug_assert!(is_aligned(size), "Size {} is not aligned", size);

        if size <= MemoryAllocator::MAX_FIXED_SIZE {
            return &self.fixed_free_lists[size / ALIGNMENT];
        } else {
            // There's probably a fancy bit manipulation way to do this
            for i in 0..MemoryAllocator::APPROX_FREE_LISTS {
                if MemoryAllocator::MAX_FIXED_SIZE << i >= size {
                    return &self.approx_free_lists[i];
                }
            }
        }

        &self.large_free_list
    }

    fn free_list_containing_mut(&mut self, size: usize) -> &mut RBTree<FreeBlockAdapter> {
        debug_assert!(is_aligned(size), "Size {} is not aligned", size);

        if size <= MemoryAllocator::MAX_FIXED_SIZE {
            return &mut self.fixed_free_lists[size / ALIGNMENT];
        } else {
            // There's probably a fancy bit manipulation way to do this
            for i in 0..MemoryAllocator::APPROX_FREE_LISTS {
                if MemoryAllocator::MAX_FIXED_SIZE << i >= size {
                    return &mut self.approx_free_lists[i];
                }
            }
        }

        &mut self.large_free_list
    }

    /// Attempt to coalesce a block with its neighbors. The block must be free
    //    fn coalesce(&mut self, block: &mut Block) {
    //        debug_assert!(!block.is_allocated(), "Block must be free");
    //
    //        if !block.prev().is_allocated() {
    //
    //        }
    //    }

    pub fn free(&mut self, payload: *mut u8) {
        // TODO: check if in heap bounds
        let mut block = unsafe { Block::from_payload(payload) };
        let block_ref = unsafe { &mut *block };

        assert!(
            block_ref.is_allocated(),
            "Attempting to free an already-freed block"
        );
        block_ref.free_link = RBTreeLink::new(); // reinitialize link
        block_ref.set_allocated(false);
        self.free_list_containing_mut(block_ref.size())
            .insert(unsafe { UnsafeRef::from_raw(block) });
    }

    pub fn allocate(&mut self, size: usize) -> Option<*mut u8> {
        let actual_size = max(Block::MIN_SIZE, align(size) + 2 * Block::TAG_SIZE);

        let free_list = self.free_list_containing_mut(actual_size);

        if let Some(block) = free_list.front_mut().remove() {
            Some(UnsafeRef::into_raw(block) as *mut u8)
        } else {
            None
        }
    }
}

// Intrusive collections are (AFAICT) impossible to make thread-safe conveniently since they use
// Cells (which are !Sync). Even Arc can't make things thread-safe, and IDK how to create an adapter
// for a Mutex-wrapped value. As long as the allocator is only accessed through the mutex in
// AllocatorMode, I'm pretty sure things should be fine.
unsafe impl Sync for MemoryAllocator {}

#[cfg(test)]
use crate::tests;

#[cfg(test)]
tests! {
    test align_correct {
        assert_eq!(align(8), 8);
        assert_eq!(align(5), 8);
        assert_eq!(align(9), 16);
        assert_eq!(align(16), 16);

        assert!(is_aligned(8));
        assert!(is_aligned(16));
        assert!(is_aligned(32));
        assert!(!is_aligned(1));
        assert!(!is_aligned(7));
        assert!(!is_aligned(29));
    }
}
