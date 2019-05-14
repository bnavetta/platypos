use core::{cmp::max, mem};

use array_init::array_init;
use bit_field::BitField;
use intrusive_collections::{
    intrusive_adapter, rbtree::CursorMut, KeyAdapter, RBTree, RBTreeLink, UnsafeRef,
};
use log::{error, trace};
use x86_64::{
    structures::paging::{Page, PhysFrame},
    VirtAddr,
};

use super::FRAME_SIZE;
use crate::kernel_state;

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

    unsafe fn create_sentinel(addr: VirtAddr) -> &'static mut Block {
        let tag = Block::TAG_SIZE | 1;
        let block = addr.as_mut_ptr::<Block>().as_mut().unwrap();
        block.tag = tag;
        block.end_tag_mut().write(tag);
        block
    }

    unsafe fn from_address(addr: VirtAddr) -> &'static mut Block {
        let block = addr.as_mut_ptr::<Block>().as_mut().unwrap();
        assert!(block.tags_match());
        block
    }

    unsafe fn from_payload(payload: *mut u8) -> &'static mut Block {
        Block::from_address(VirtAddr::from_ptr(payload) - Block::TAG_SIZE as u64)
    }

    /// Returns this block's address in memory
    fn address(&self) -> VirtAddr {
        VirtAddr::from_ptr(self as *const Block)
    }

    /// Returns the address of the first byte after this block
    fn end_address(&self) -> VirtAddr {
        self.address() + self.size()
    }

    fn payload(&self) -> *mut u8 {
        let ptr = (self as *const Block) as *mut u8;
        unsafe { ptr.add(Block::TAG_SIZE) }
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

    // TODO: should prev/next assert they're not the prologue/epilogue?

    /// Get a reference to the next block in memory
    fn next(&self) -> &'static Block {
        let ptr = (self as *const Block) as *const u8;
        unsafe {
            let next_ptr = ptr.add(self.size());
            (next_ptr as *const Block).as_ref().unwrap()
        }
    }

    /// Get a mutable reference to the next block in memory
    fn next_mut(&self) -> &'static mut Block {
        let ptr = (self as *const Block) as *const u8;
        unsafe {
            let next_ptr = ptr.add(self.size());
            (next_ptr as *mut Block).as_mut().unwrap()
        }
    }

    /// Get a reference to the previous block in memory
    fn prev(&self) -> &'static Block {
        let ptr = (self as *const Block) as *const usize;
        unsafe {
            let prev_end_tag = ptr.offset(-1).read();
            let prev_size = prev_end_tag & !1;
            let next_ptr = (ptr as *const u8).sub(prev_size);
            (next_ptr as *const Block).as_ref().unwrap()
        }
    }

    /// Get a mutable reference to the previous block in memory
    fn prev_mut(&self) -> &'static mut Block {
        let ptr = (self as *const Block) as *const usize;
        unsafe {
            let prev_end_tag = ptr.offset(-1).read();
            let prev_size = prev_end_tag & !1;
            let next_ptr = (ptr as *const u8).sub(prev_size);
            (next_ptr as *mut Block).as_mut().unwrap()
        }
    }

    fn free_link(&self) -> &RBTreeLink {
        debug_assert!(!self.is_allocated());
        &self.free_link
    }

    fn as_ref(&self) -> UnsafeRef<Block> {
        unsafe { UnsafeRef::from_raw(self as *const Block) }
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
    free_lists: [RBTree<FreeBlockAdapter>;
        MemoryAllocator::FIXED_FREE_LISTS + MemoryAllocator::APPROX_FREE_LISTS + 1],

    // The allocatable portion of memory is between heap_start and heap_end. This region is allowed
    // to extend up until heap_max. This entire portion of the address space is reserved for the
    // allocator and can't be used for anything else. heap_end/heap_max are exclusive, heap_start is
    // inclusive
    heap_start: VirtAddr,
    heap_end: VirtAddr,
    heap_max: VirtAddr,

    // Range of memory used for the bootstrap heap, which we need to make sure not to free
    bootstrap_start: VirtAddr,
    bootstrap_end: VirtAddr,
}

impl MemoryAllocator {
    const MAX_FIXED_SIZE: usize = 512; // largest size that goes in a fixed free list
    const FIXED_FREE_LISTS: usize = MemoryAllocator::MAX_FIXED_SIZE / ALIGNMENT;
    const APPROX_FREE_LISTS: usize = 4;
    const EXTEND_PAGES: usize = 4; // How many pages at a time to extend the heap by

    pub fn new(
        heap_start: VirtAddr,
        heap_max: VirtAddr,
        bootstrap_start: VirtAddr,
        bootstrap_end: VirtAddr,
    ) -> Option<MemoryAllocator> {
        assert!(
            heap_max > heap_start,
            "Heap maximum {:?} is below heap start {:?}",
            heap_max,
            heap_start
        );
        assert!(heap_start.is_aligned(FRAME_SIZE as u64));
        assert!(heap_max.is_aligned(FRAME_SIZE as u64));
        assert!(bootstrap_end > bootstrap_start);
        assert!(
            bootstrap_end <= heap_start || bootstrap_start >= heap_max,
            "Bootstrap and main heaps overlap"
        );

        // TODO: arr! macro seemed nicer, but procedural macros don't seem to work with cargo-xbuild
        let mut alloc = MemoryAllocator {
            free_lists: array_init(|_| RBTree::new(FreeBlockAdapter::new())),
            heap_start,
            heap_end: heap_start,
            heap_max,
            bootstrap_start,
            bootstrap_end,
        };

        if let Some(start) = alloc.add_pages(1) {
            assert_eq!(start, heap_start, "First pages of heap should be at heap_start");
        } else {
            return None;
        }

        // Create the two sentinel blocks. The advantage of these is that they make .prev/.next safer,
        // since they won't progress past the end of the heap. Because we can only allocate in
        // page-sized chunks, there's also an initial free block
        let prologue = unsafe { Block::create_sentinel(heap_start)};

        let first_free = prologue.next_mut();
        first_free.set_size(FRAME_SIZE - 4 * Block::TAG_SIZE);
        first_free.set_allocated(false);

        unsafe { Block::create_sentinel(VirtAddr::from_ptr(first_free.next_mut() as *mut Block)); }

        alloc.insert_free_block(first_free);

        Some(alloc)
    }

    /// Returns the index in the free list array for the list responsible for blocks of the given size
    fn free_list_index(&self, size: usize) -> usize {
        debug_assert!(is_aligned(size), "Size {} is not aligned", size);

        if size <= MemoryAllocator::MAX_FIXED_SIZE {
            size / ALIGNMENT
        } else if size <= MemoryAllocator::MAX_FIXED_SIZE << MemoryAllocator::APPROX_FREE_LISTS {
            // There's probably a fancy bit manipulation way to do this
            for i in 0..MemoryAllocator::APPROX_FREE_LISTS {
                if MemoryAllocator::MAX_FIXED_SIZE << i >= size {
                    return MemoryAllocator::FIXED_FREE_LISTS + i;
                }
            }

            unreachable!("Size {} should be in approximate free-list region", size);
        } else {
            self.free_lists.len() - 1
        }
    }

    fn free_cursor_to(&mut self, block: &mut Block) -> CursorMut<FreeBlockAdapter> {
        assert!(
            !block.is_allocated(),
            "Allocated blocks are not in the free list"
        );
        assert!(
            block.free_link().is_linked(),
            "Block is not in the free list"
        );
        unsafe {
            self.free_lists[self.free_list_index(block.size())]
                .cursor_mut_from_ptr(block as *const Block)
        }
    }

    fn insert_free_block(&mut self, block: &mut Block) {
        assert!(
            !block.is_allocated(),
            "Cannot add an allocated block to the free list"
        );

        assert!(
            block.address() >= self.heap_start,
            "Block lies outside the heap"
        );
        assert!(
            block.end_address() <= self.heap_end,
            "Block lies outside the heap"
        );

        assert!(block.tags_match(), "Block tags don't match");

        // Try to coalesce with the next block
        if !block.next().is_allocated() {
            let next = block.next_mut();
            self.free_cursor_to(next).remove();
            block.set_size(block.size() + next.size());
        }

        // Try to coalesce with the previous block
        if block.prev().is_allocated() {
            block.free_link = RBTreeLink::new(); // Reinitialize link, since it could have been used as payload
            self.free_lists[self.free_list_index(block.size())].insert(block.as_ref());
        } else {
            let prev = block.prev_mut();
            prev.set_size(prev.size() + block.size());
            // prev should already in the free list
            assert!(
                prev.free_link().is_linked(),
                "Block should be in the free list"
            );
        }
    }

    fn allocate_block(
        &mut self,
        block: &'static mut Block,
        needed_size: usize,
    ) -> &'static mut Block {
        assert!(
            !block.is_allocated(),
            "Cannot allocate an already-allocated block"
        );
        assert!(
            !block.free_link().is_linked(),
            "Cannot allocate a block in the free list"
        );

        assert!(
            block.address() >= self.heap_start,
            "Block lies outside the heap"
        );
        assert!(
            block.end_address() <= self.heap_end,
            "Block lies outside the heap"
        );

        assert!(block.tags_match(), "Block tags don't match");

        block.set_allocated(true);

        let extra_size = block.size() - needed_size;
        // TODO: don't always split?
        if extra_size > Block::MIN_SIZE {
            block.set_size(needed_size);
            let next = block.next_mut();
            next.set_size(extra_size);
            next.set_allocated(false);
            self.insert_free_block(next);
        }

        block
    }

    pub fn free(&mut self, payload: *mut u8) {
        let payload_addr = VirtAddr::from_ptr(payload);
        if payload_addr >= self.bootstrap_start && payload_addr < self.bootstrap_end {
            trace!(
                "Leaking bootstrap allocation at {:#x}",
                payload_addr.as_u64()
            );
            return;
        }

        assert!(payload_addr >= self.heap_start);
        assert!(payload_addr < self.heap_end);

        let block = unsafe { Block::from_payload(payload) };
        trace!(
            "Freeing a {}-byte block at {:#x}",
            block.size(),
            block.address().as_u64()
        );

        assert!(
            block.is_allocated(),
            "Attempting to free an already-freed block"
        );
        block.set_allocated(false);
        self.insert_free_block(block);
    }

    pub fn allocate(&mut self, size: usize) -> Option<*mut u8> {
        let actual_size = max(Block::MIN_SIZE, align(size) + 2 * Block::TAG_SIZE);

        for i in self.free_list_index(actual_size)..self.free_lists.len() {
            if let Some(block) = self.free_lists[i].front_mut().remove() {
                unsafe {
                    let block = UnsafeRef::into_raw(block);

                    let block = self.allocate_block(block.as_mut().unwrap(), actual_size);
                    trace!(
                        "Allocating a {}-byte block at {:#x}",
                        block.size(),
                        block.address().as_u64()
                    );
                    return Some(block.payload());
                }
            }
        }

        if self.extend_heap() {
            // use recursion to avoid duplicating the search loop
            self.allocate(size)
        } else {
            None
        }
    }

    /// Extend the heap by a fixed amount, shifting the epilogue as necessary. This can only be
    /// called once the heap has been set up.
    fn extend_heap(&mut self) -> bool {
        if let Some(old_end) = self.add_pages(MemoryAllocator::EXTEND_PAGES) {
            let epilogue = unsafe { Block::from_address(old_end) }.prev_mut();

            epilogue.set_size(MemoryAllocator::EXTEND_PAGES * FRAME_SIZE);
            epilogue.set_allocated(false);

            // create new epilogue
            unsafe { Block::create_sentinel(VirtAddr::from_ptr(epilogue.next_mut() as *mut Block)); }

            self.insert_free_block(epilogue);

            true
        } else {
            false
        }
    }

    /// Adds pages to the heap, returning the old heap end. Returns `None` if allocation failed.
    /// This does not adjust the epilogue at all, and can be used to create the initial heap.
    fn add_pages(&mut self, npages: usize) -> Option<VirtAddr> {
        let new_end = self.heap_end + FRAME_SIZE * npages;
        if new_end > self.heap_max {
            return None;
        }

        trace!("Extending heap by {} pages", npages);

        if let Some(memory) = kernel_state()
            .frame_allocator()
            .allocate_pages(npages)
        {
            if !kernel_state().with_page_table(|pt| {
                let phys_start = PhysFrame::containing_address(
                    pt.translate(VirtAddr::from_ptr(memory))
                        .expect("Could not translate allocated page frames"),
                );
                match unsafe {
                    pt.map_contiguous(
                        Page::range(
                            Page::containing_address(self.heap_end),
                            Page::containing_address(new_end),
                        ),
                        PhysFrame::range(
                            phys_start,
                            phys_start + npages as u64,
                        ),
                        true,
                    )
                } {
                    Ok(()) => true,
                    Err(e) => {
                        error!("Error mapping new page frames into heap: {:?}", e);
                        kernel_state()
                            .frame_allocator()
                            .free_pages(npages, memory);
                        false
                    }
                }
            }) {
                return None;
            }

            let old_end = self.heap_end;
            self.heap_end = new_end;
            Some(old_end)

//            let block = unsafe {
//                Block::initialize(
//                    self.heap_end.as_mut_ptr(),
//                    MemoryAllocator::EXTEND_PAGES * FRAME_SIZE,
//                    false,
//                )
//            };
//            self.heap_end = new_end;
//            self.insert_free_block(block);
//
//            true
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
