//!
//! Physical page allocator using buddy bitmaps.
//!
//! Physical memory is split into contiguous ranges called `Region`s. Each region has a bitmap tree
//! of a fixed depth of 12, with 8 entries at the top level (which ensures that the entire tree fits
//! in a page). If the bitmap tree describes more memory than the region actually contains, the
//! excess is permanently marked as allocated. Alternatively, there can be multiple directly adjacent
//! regions if there's a chunk of memory too big for a single region to represent.
//!
//! There are a couple useful properties for figuring out how the bitmap is laid out. Consider an
//! order _k_:
//! * The bitmap takes up `2^(MAX_ORDER - k)` bytes
//! * The bitmap holds `2^(MAX_ORDER - k + 3)` entries
//! * The bitmap starts `2^(MAX_ORDER - k) - 1` bytes from the start of the tree
//!
use core::cmp::min;
use core::marker::PhantomData;
use core::mem;
use core::ptr;
use core::slice;

use bit_field::BitArray;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};
use log::trace;
use spin::Mutex;

use hal::{Platform, PhysicalAddress, VirtualAddress};

/// A physical memory allocator using buddy bitmaps. Note that, despite being a physical memory
/// allocator, `FrameAllocator` deals entirely in virtual addresses within the kernel's physical
/// memory map region. This is partly for ease of implementation, as the allocator needs access
/// to bookkeeping information, and partly for caller convenience, as most callers will need to
/// manipulate the allocated memory. The implementation guarantees, however, that all returned
/// virtual addresses do lie within the physical memory map.
pub struct FrameAllocator<P: Platform> {
    regions: LinkedList<RegionAdapter<P>>
}

impl <P: Platform> FrameAllocator<P> {
    pub const fn new() -> FrameAllocator<P> {
        FrameAllocator {
            regions: LinkedList::new(RegionAdapter::new())
        }
    }

    /// Add a region of allocatable physical memory for this allocator to use.
    ///
    /// # Arguments
    /// * `start` - the virtual address in the kernel's physical memory map of the start of the region
    /// * `num_frames` -
    pub fn add_range(&mut self, start: VirtualAddress, num_frames: usize) {

    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct FrameAllocation {
    start: VirtualAddress,
    num_frames: usize
}

// IMPLEMENTATION

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
struct Order(u8);

impl Order {
    const MAX: Order = Order(11);
    const MAX_VAL: usize = 11;

    const MIN: Order = Order(0);

    /// Returns the number of frames in a block of this order
    const fn frames(&self) -> usize {
        1usize << self.0
    }

    /// Returns the maximum allowed index for a block of this order. This relies on assuming a
    /// one-page tree, where the order-11 bitmap occupies 1 byte
    const fn max_index(&self) -> usize {
        // See the properties for orders listed above
        1 << (Order::MAX_VAL - self.as_usize() + 3)
    }

    fn parent(&self) -> Order {
        debug_assert!(*self < Order::MAX);
        Order(self.0 + 1)
    }

    fn child(&self) -> Order {
        debug_assert!(self.0 > 0);
        Order(self.0 - 1)
    }

    const fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl From<usize> for Order {
    fn from(v: usize) -> Order {
        debug_assert!(v <= Order::MAX_VAL);
        Order(v as u8)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
struct BlockId {
    order: Order,
    index: usize,
}

impl BlockId {
    fn new(order: Order, index: usize) -> BlockId {
        debug_assert!(index <= order.max_index());
        BlockId { order, index }
    }

    #[inline(always)]
    fn order(&self) -> Order {
        self.order
    }

    #[inline(always)]
    fn index(&self) -> usize {
        self.index
    }

    #[inline(always)]
    fn sibling(&self) -> BlockId {
        BlockId::new(
            self.order,
            if self.index % 2 == 0 {
                self.index + 1
            } else {
                self.index - 1
            },
        )
    }

    #[inline(always)]
    fn parent(&self) -> Option<BlockId> {
        if self.order < Order::MAX {
            let parent = (self.index & !1) >> 1;
            Some(BlockId::new(self.order.parent(), parent))
        } else {
            None
        }
    }

    #[inline(always)]
    fn left_child(&self) -> BlockId {
        debug_assert!(self.order > Order::MIN);
        BlockId::new(self.order.child(), self.index << 1)
    }

    #[inline(always)]
    fn right_child(&self) -> BlockId {
        debug_assert!(self.order > Order::MIN);
        BlockId::new(self.order.child(), (self.index << 1) + 1)
    }
}

struct Region<P: Platform> {
    /// Link in the parent FrameAllocator
    link: LinkedListLink,

    /// Inner, mutable state
    inner: Mutex<RegionInner<P>>,
}

/// Inner, mutable state for a region. This layering is necessary because intrusive collection
/// members need to be immutable (see https://github.com/Amanieu/intrusive-rs/issues/19). To get
/// around this, all the mutable bits go in an inner struct, which the outer one wraps in a Mutex.
/// Then, we can pass around &'static Region pointers and keep all the state for a region in its
/// first page.
struct RegionInner<P: Platform> {
    /// Number of physical page frames
    num_frames: usize,

    /// We hold on to this here for convenience - it's nicer than doing sketchy pointer arithmetic
    region_start: VirtualAddress,

    // Bitmap tree
    bitmaps: [&'static mut [u8]; Order::MAX_VAL + 1],

    // Free lists for each order
    free_lists: [LinkedList<FreeBlockAdapter>; Order::MAX_VAL + 1],

    platform: PhantomData<P>
}

impl <P: Platform> RegionInner<P> {
    // Size of the region header (Region struct and bitmaps)
    fn header_size() -> usize {
        2 * P::PAGE_SIZE
    }

    #[inline]
    fn start_addr(&self) -> VirtualAddress {
        self.region_start
    }

    #[inline]
    fn end_addr(&self) -> VirtualAddress {
        self.region_start + (self.num_frames * P::PAGE_SIZE)
    }

    // NOTE: to ensure the header is _never_ allocated and make initialization a bit easier, it's
    // not part of the allocatable region. Thus, the start of the data portion of the region starts
    // 2 pages after the region start.

    #[inline]
    fn data_start_addr(&self) -> VirtualAddress {
        self.region_start + Self::header_size()
    }

    #[inline]
    fn data_frames(&self) -> usize {
        self.num_frames as usize - 2
    }

    #[inline]
    fn mark_allocated(&mut self, block: BlockId, allocated: bool) {
        self.bitmaps[block.order().as_usize()].set_bit(block.index(), allocated)
    }

    #[inline]
    fn is_allocated(&self, block: BlockId) -> bool {
        self.bitmaps[block.order().as_usize()].get_bit(block.index())
    }

    fn block_address(&self, block: BlockId) -> VirtualAddress {
        debug_assert!(
            block.order() <= self.max_order(),
            "Block does not fit in region"
        );
        debug_assert!(
            block.index() <= self.max_index(block.order()),
            "Block does not fit in region"
        );
        self.data_start_addr() + block.index() * block.order().frames() * P::PAGE_SIZE
    }

    fn block_id(&self, addr: VirtualAddress, order: Order) -> BlockId {
        debug_assert!(
            self.contains(addr),
            "Block {:?} does not belong to region",
            addr
        );
        debug_assert!(
            addr.is_aligned(P::PAGE_SIZE),
            "Address must be page-aligned"
        );

        let index = usize::from(addr - self.data_start_addr()) / P::PAGE_SIZE / order.frames();
        BlockId::new(order, index)
    }

    /// Returns the highest index within this region for a block of the given order
    fn max_index(&self, order: Order) -> usize {
        // subtract 1 since 0-indexed
        self.data_frames() / order.frames() - 1
    }

    /// Returns the largest order which can be allocated in this region
    fn max_order(&self) -> Order {
        Order::from(min(log2(self.data_frames()), Order::MAX_VAL))
    }

    fn free(&mut self, block: BlockId) {
        debug_assert!(
            self.is_allocated(block),
            "Freeing a block that isn't allocated"
        );

        if let Some(parent) = block.parent() {
            // Not at the top, so we can try merging with our sibling
            if self.is_allocated(block.sibling()) {
                let free_block =
                    unsafe { FreeBlock::create_at(self.block_address(block), block.order()) };
                self.free_lists[block.order().as_usize()].push_front(free_block);
                self.mark_allocated(block, false);
                trace!("Freed {:?}", block);
            } else {
                assert!(
                    self.is_allocated(parent),
                    "Parent of allocated block must be allocated"
                );

                // Need to un-free sibling for merging
                self.mark_allocated(block.sibling(), true);
                let sibling =
                    unsafe { FreeBlock::from_address(self.block_address(block.sibling())) };
                debug_assert!(sibling.order == block.order(), "Sibling has wrong order");
                debug_assert!(
                    sibling.link.is_linked(),
                    "Sibling should be in the free list"
                );
                unsafe { self.free_lists[block.order().as_usize()].cursor_mut_from_ptr(sibling) }
                    .remove();

                self.free(parent);
            }
        } else {
            let free_block =
                unsafe { FreeBlock::create_at(self.block_address(block), block.order()) };
            self.free_lists[block.order().as_usize()].push_front(free_block);
            self.mark_allocated(block, false);
            trace!("Freed {:?}", block);
        }
    }

    fn alloc(&mut self, order: Order) -> Option<BlockId> {
        if let Some(block) = self.free_lists[order.as_usize()].pop_front() {
            debug_assert!(block.order == order);
            let block_id = self.block_id(VirtualAddress::from_pointer(block), order);
            self.mark_allocated(block_id, true);
            trace!("Allocating {:?}", block_id);
            Some(block_id)
        } else if order < Order::MAX {
            if let Some(parent) = self.alloc(order.parent()) {
                let block = parent.left_child();
                let sibling = parent.right_child();

                self.mark_allocated(block, true);
                self.free(sibling);
                trace!("Allocating {:?}", block);
                Some(block)
            } else {
                None
            }
        } else {
            None
        }
    }

    fn contains(&self, addr: VirtualAddress) -> bool {
        addr >= self.start_addr() && addr < self.end_addr()
    }
}

impl <P: Platform> Region<P> {
    const MAX_FRAMES: usize = 8 * Order::MAX.frames();

    fn new(start: VirtualAddress, num_frames: usize) -> &'static Region<P> {
        assert!(
            num_frames > 2,
            "Region of size {} is not large enough",
            num_frames
        );
        assert!(
            num_frames <= Self::MAX_FRAMES,
            "Region cannot support {} frames",
            num_frames
        );

        let region: &'static mut Region<P> = unsafe { start.as_mut_ref() };
        region.link = LinkedListLink::new();
        // TODO: Using mem::zeroed() is kinda hacky, probably better to fully initialize RegionInner before setting on the Region
        region.inner = Mutex::new(unsafe { mem::zeroed() });

        let mut inner = region.inner.lock();

        inner.num_frames = num_frames;
        inner.region_start = start;

        // Bitmaps start in the second page of the region
        let bitmaps_start = start + P::PAGE_SIZE;

        for order in 0..=Order::MAX_VAL {
            let bitmap_size = 1 << (Order::MAX_VAL - order);
            let bitmap_addr = bitmaps_start + (bitmap_size - 1usize);

            inner.bitmaps[order] = unsafe {
                let start_ptr = bitmap_addr.as_mut_pointer();
                ptr::write_bytes(start_ptr, 0xff, bitmap_size as usize); // Mark everything as allocated
                slice::from_raw_parts_mut(start_ptr, bitmap_size as usize)
            };

            inner.free_lists[order] = LinkedList::new(FreeBlockAdapter::new());
        }

        let avail_frames = num_frames - 2; // Header is 2 pages
        let mut frame_start = 0;
        let mut order = Order::MAX;

        while frame_start < avail_frames {
            let remaining = avail_frames - frame_start;
            if order.frames() <= remaining as usize {
                inner.bitmaps[order.as_usize()]
                    .set_bit(frame_start as usize >> order.as_usize(), false);

                let block = unsafe {
                    FreeBlock::create_at(start + ((frame_start + 2) * P::PAGE_SIZE), order)
                };
                inner.free_lists[order.as_usize()].push_back(block);

                frame_start += order.frames();
            } else {
                order = order.child();
            }
        }

        region
    }

    fn alloc(&self, order: usize) -> Option<VirtualAddress> {
        let mut inner = self.inner.lock();
        inner.alloc(order.into()).map(|id| inner.block_address(id))
    }

    fn free(&self, order: usize, block: VirtualAddress) {
        let mut inner = self.inner.lock();

        let id = inner.block_id(block, order.into());
        inner.free(id);
    }

    fn contains(&self, addr: VirtualAddress) -> bool {
        let inner = self.inner.lock();
        inner.contains(addr)
    }
}

// Region otherwise wouldn't be Sync because LinkedListLink isn't Sync. This honestly seems like an
// issue with intrusive_collections - LinkedList is supposed to be Sync if the value type is Sync,
// but since LinkedListLink uses Cell, it seems like the value type never _can_ be Sync. Even using
// Arc doesn't seem like it'd work, since the constraint is on the value type. It should be fine
// here - the linked list of Regions is only modified when initializing the system, and the mutable
// parts (RegionInner) are wrapped in a Mutex anyways.
unsafe impl <P: Platform> Sync for Region<P> {}

struct FreeBlock {
    link: LinkedListLink,
    order: Order, // for debugging
}

impl FreeBlock {
    unsafe fn from_address(addr: VirtualAddress) -> &'static FreeBlock {
        addr.as_ref()
    }

    unsafe fn create_at(addr: VirtualAddress, order: Order) -> &'static FreeBlock {
        let block: &'static mut FreeBlock = addr.as_mut_ref();
        block.link = LinkedListLink::new();
        block.order = order;
        block
    }
}

intrusive_adapter!(FreeBlockAdapter = &'static FreeBlock : FreeBlock { link: LinkedListLink });
intrusive_adapter!(RegionAdapter<P> = &'static Region<P> : Region<P> { link: LinkedListLink } where P: Platform);

/// Computes the integer part of the base-2 logarithm of x
const fn log2(x: usize) -> usize {
    // https://en.wikipedia.org/wiki/Find_first_set
    (mem::size_of::<usize>() * 8) - 1 - (x.leading_zeros() as usize)
}