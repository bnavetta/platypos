//! The "root" kernel allocator. This allocator is responsible for managing
//! physical memory, and so it underpins all other allocators in the system.
//!
//! The allocator borrows an approach from Fuschia, where all usable memory is
//! tracked by a linked list of ranges. Each range is tagged with its status
//! (free, used, etc.). Memory for the linked list is itself allocated out of
//! the usable region. This allows efficiently summarizing the allocation state
//! of memory. It's also reasonably performant when allocating (searching the
//! list for a big enough range) and allows allocation policies (like "must be
//! below a certain address"). However, deallocation is slow - we have to scan
//! the list to find out which range the allocation came from. Unlike Fuschia,
//! ranges cannot overlap.

use core::alloc::Layout;
use core::cell::RefCell;
use core::fmt;
use core::mem::{self, MaybeUninit};

use alloc::vec::Vec;

use intrusive_collections::linked_list::CursorMut;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink, UnsafeRef};
use linked_list_allocator::LockedHeap;

use crate::arch::mm::MemoryAccess;
use crate::prelude::*;

use super::map::Region;

/// Builder for the physical memory allocator
pub struct Builder;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
enum Status {
    /// Memory that can be allocated
    Free,
    /// Memory that is already allocated
    Allocated,
    /// Memory used for internal bookkeeping
    Tracking,
    /// This is an unused but available run
    Unused,
}

/// A run of usable memory
#[derive(Debug)]
struct Run {
    /// Link in the free list. Only set if the status is [`Free`]
    free_link: LinkedListLink,

    /// Link in the overall range list
    link: LinkedListLink,

    /// Mutable state of a Run
    /// See https://github.com/Amanieu/intrusive-rs/issues/38
    inner: RefCell<RunState>,
}

#[derive(Debug)]
struct RunState {
    /// Range of memory that this run describes
    range: PageFrameRange,

    /// Status of this range
    status: Status,
}

intrusive_adapter!(RunAdapter = UnsafeRef<Run>: Run { link: LinkedListLink });
intrusive_adapter!(FreeRunAdapter = UnsafeRef<Run>: Run { free_link: LinkedListLink });

/// Size in page frames of the scratch region to allocate for bookkeeping while
/// configuring the allocator
const SCRATCH_PAGES: usize = 2;

/// Minimum number of pages to allocate towards tracking memory
const MIN_TRACKING_PAGES: usize = 2;

impl Builder {
    pub fn parse_memory_map<I>(
        &mut self,
        access: &mut MemoryAccess,
        memory_map: I,
        reserved: &[PageFrameRange],
    ) -> Result<(), Error>
    where
        I: Iterator<Item = Region> + Clone,
    {
        defmt::info!("Initializing physical memory allocator");

        // First, find a scratch region:
        let (scratch, initial_tracking) = memory_map
            .clone()
            .find_map(|r| {
                if r.usable() && r.size() >= ((MIN_TRACKING_PAGES + SCRATCH_PAGES) * PAGE_SIZE) {
                    for reserved_region in reserved.iter() {
                        if reserved_region.address_range().intersects(&r.range()) {
                            return None;
                        }
                    }

                    let start_pf = PageFrame::from_start(r.start())
                        .expect("Memory region is not page-aligned!");

                    let initial_tracking = PageFrameRange::from_start_size(start_pf, SCRATCH_PAGES);
                    let scratch =
                        PageFrameRange::from_start_size(initial_tracking.end(), MIN_TRACKING_PAGES);
                    Some((scratch, initial_tracking))
                } else {
                    None
                }
            })
            .ok_or(Error::new(ErrorKind::InsufficientMemory))?;

        defmt::debug!("Scratch space: {}", scratch.address_range());
        defmt::debug!(
            "Initial tracking pages: {}",
            initial_tracking.address_range()
        );

        unsafe {
            access.with_memory::<_, Result<(), Error>>(scratch, |access, s| {
                // LockedHeap expects that the passed-in region is 'static, but the allocator
                // and allocations made with it don't escape this block, so we should be ok.
                let alloc = LockedHeap::new(s.as_mut_ptr().cast(), s.len());

                let mut ranges = Vec::new_in(&alloc);

                for region in memory_map {
                    if region.usable() {
                        let start = PageFrame::from_start(region.start())
                            .expect("Memory region is not page-aligned!");
                        defmt::assert!(
                            region.size() % PAGE_SIZE == 0,
                            "Region size is not a whole number of pages!"
                        );
                        let size = region.size() / PAGE_SIZE;
                        let mut range = PageFrameRange::from_start_size(start, size);
                        // Make sure the initial tracking memory isn't double-allocated. This works
                        // because we allocate the tracking and scratch ranges from the _start_ of a
                        // usable range. Note that the scratch range is not removed here, so it will
                        // become usable once the allocator is initialized.
                        if range.start() == initial_tracking.start() {
                            range.shrink_left(MIN_TRACKING_PAGES);
                        }

                        ranges.push(range);
                    }
                }

                ranges.shrink_to_fit();
                ranges.sort_unstable_by_key(|r| r.start());

                // Combine overlapping memory ranges
                // Look! The interview question came in handy :P
                let mut i = 1; // We know there's at least one usable region
                while i < ranges.len() {
                    let prev = &ranges[i - 1];
                    let cur = &ranges[i];
                    if prev.intersects(cur) {
                        let new_size = cur.end() - prev.start();
                        ranges.remove(i);
                        // Re-borrow mutably here to avoid a split borrow
                        (&mut ranges[i - 1]).set_size(new_size);
                    } else {
                        i += 1;
                    }
                }

                // TODO: remove reserved ranges

                defmt::debug!("Usable memory:");
                for range in &ranges {
                    defmt::debug!(" - {}", range.address_range());
                }

                let mut allocator = AllocatorInner::new();
                allocator.init_tracking_space(access, initial_tracking)?;
                for range in &ranges {
                    allocator.add_allocatable_range(*range);
                }

                defmt::info!(
                    "Post-initialization allocator state:\n{}",
                    // TODO: migrate to defmt
                    defmt::Display2Format(&allocator.display_state())
                );

                let small_allocation = allocator.allocate(4).unwrap();
                let big_allocation = allocator.allocate(1000).unwrap();
                defmt::info!(
                    "Small allocation: {}\nBig allocation: {}",
                    small_allocation,
                    big_allocation
                );
                defmt::info!(
                    "Allocator state after allocations:\n{}",
                    defmt::Display2Format(&allocator.display_state())
                );

                /*


                // Ending page frame number is the number of page frames we have to track
                let page_frames = ranges.last().unwrap().end().as_usize();
                let bytes_needed = (page_frames / PAGES_PER_BLOCK).div_ceil(u8::BITS as usize);

                let bitmap_location = ranges
                    .iter()
                    .find_map(|r| {
                        if r.size_bytes() >= bytes_needed {
                            Some(PageFrameRange::from_start_size(
                                r.start(),
                                bytes_needed.div_ceil(PAGE_SIZE),
                            ))
                        } else {
                            None
                        }
                    })
                    .ok_or(Error::new(ErrorKind::InsufficientMemory))?;

                log::debug!("Placing bitmap at {}", bitmap_location.address_range());

                */

                // Block approach has a big fragmentation problem:
                // - can't use small usable regions < 2MiB
                // - annoying to use regions that don't start on a 2MiB boundary

                // Solution?
                // - allocator manages individual pages
                // - scan bitmap to find big chunks, pull those out, and put them in a separate
                //   free list
                // - can always add big chunks back to pool if needed
                // LK (used in Fuschia) finds contiguous runs as needed: https://github.com/littlekernel/lk/blob/master/kernel/vm/pmm.c#L283
                // But Fuschia does something more complicated? https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/kernel/phys/lib/memalloc/pool-test.cc
                // Linked list of runs of free pages!?
                // https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/kernel/phys/lib/memalloc/include/lib/memalloc/pool.h

                drop(ranges);
                drop(alloc);

                Ok(())
            })??;
            // Have outer Allocator hold the MemoryAccess - can create outside
            // the map_temporary block
        }

        /*

                // This implicitly assumes that there isn't a large memory hole in low memory.
                // If there were, we might not want to waste space marking it as unusable.

                // Need one bit per page
                let page_count = max_usable.as_usize().div_ceil(PAGE_SIZE);
                let bitmap_size = page_count / u8::BITS as usize;
                log::info!("Need {} for bitmap", bitmap_size.as_size());

                let bitmap_region = memory_map
                    .filter(|r| r.usable())
                    .coalesce(|a, b| {
                        // Combine adjacent usable regions. It's possible that the firmware provided a
                        // very fragmented memory map, so coalescing makes sure that the first usable
                        // bitmap space is found.
                        if b.start() <= a.end() {
                            Ok(Region::new(a.kind(), a.start(), b.end()))
                        } else {
                            Err((a, b))
                        }
                    })
                    .find(|r| r.size() >= bitmap_size)
                    .ok_or(Error::new(ErrorKind::InsufficientMemory))?;

                log::info!("Using {} for bitmap", bitmap_region);

                let bitmap = unsafe {
                    // Safety: all usable memory is unallocated at this point, so there
                    // is not an existing mapping to alias TODO: better PPN
                    // type

                    //let addr = access.map_permanent(todo!("PPN
                    // API"), bitmap_size.div_ceil(PAGE_SIZE))?; TODO: make
                    // sure we have a whole-usize allocation - may need to
                    // round up bitmap_size for that
                };

        */

        Ok(())

        // TODOs:
        // - API for temporarily accessing physical memory (could return a &'a
        //   [MaybeUninit<u8>] to ensure reference doesn't escape)
        // - alternative construction algorithm to handle unusable memory like
        //   kernel: 1. Find a usable page frame (in list from bootloader,
        //   doesn't overlap w/ reserved regions) 2. Use that to create a
        //   scratch allocator 3. Put memory into a Vec in scratch allocator -
        //   then can sort, coalesce, split out reserved regions
        // - clean up xtasks:
        //     - support building multiple platforms at once
        //     - more composable abstractions:
        //         - build crate X for platform Y, producing the binary artifact
        //         - run kernel-like artifact (platform-appropriate) on platform
        //           Y (platform knows which qemu to use, can pass generic
        //           settings like memory+CPUs)
        //     - generally, hide platform-specific bootloader stuff better
    }
}

struct AllocatorInner {
    // Often, an allocator method has a cursor into the runs list, and want to call some other
    // method with that cursor. Unfortunately, the cursor has a mutable reference to the runst
    // list, so we can't call other mutable methods - the borrow checker doesn't know that those
    // methods won't _also_ use runs. Something like view types (https://smallcultfollowing.com/babysteps/blog/2021/11/05/view-types/)
    // would solve the problem. For now, put all the non-runs fields in a separate struct, and use
    // associated functions instead of methods.
    runs: LinkedList<RunAdapter>,
    tracking: AllocatorTracking,
}

struct AllocatorTracking {
    /// List of runs identifying free RAM
    free: LinkedList<FreeRunAdapter>,
    /// List of allocated-but-unused [`Run`] structures
    unused_runs: LinkedList<RunAdapter>,
}

struct DisplayAllocatorState<'a> {
    allocator: &'a AllocatorInner,
}

impl AllocatorInner {
    fn new() -> Self {
        AllocatorInner {
            runs: LinkedList::new(RunAdapter::new()),
            tracking: AllocatorTracking {
                free: LinkedList::new(FreeRunAdapter::new()),
                unused_runs: LinkedList::new(RunAdapter::new()),
            },
        }
    }

    fn allocate(&mut self, count: usize) -> Result<PageFrameRange, Error> {
        // TODO: ensure runs available

        // First-fit algorithm, could add other conditions (e.g. must allocate below a
        // certain address for hardware reasons)
        // let allocatable_run = self
        //     .tracking
        //     .free
        //     .iter()
        //     .find(|r| r.size() >= count)
        //     .ok_or_else(|| Error::new(ErrorKind::InsufficientMemory))?;
        let mut free_cursor = {
            let mut free_cursor = self.tracking.free.front_mut();
            loop {
                match free_cursor.get() {
                    Some(free_run) => {
                        if free_run.size() >= count {
                            break free_cursor;
                        } else {
                            free_cursor.move_next();
                        }
                    }
                    None => return Err(Error::new(ErrorKind::InsufficientMemory)),
                }
            }
        };
        let allocatable_run = free_cursor.get().unwrap();

        if allocatable_run.size() == count {
            // If we're using the whole run, just mark it directly instead of trying to
            // split it up and do a no-op coalesce
            let range = {
                let mut run_state = allocatable_run.inner.borrow_mut();
                run_state.status = Status::Allocated;
                run_state.range
            };
            free_cursor.remove();
            Ok(range)
        } else {
            // Split the allocation off the start of the run, so that we can reuse it as the
            // cursor for adding the allocated run
            let range = {
                let mut allocatable_inner = allocatable_run.inner.borrow_mut();
                let range = PageFrameRange::from_start_size(allocatable_inner.range.start(), count);
                allocatable_inner.range.shrink_left(count);
                range
            };

            let allocated_run = self
                .tracking
                .unused_runs
                .pop_front()
                .expect("TODO: add new tracking runs as needed");
            allocated_run.initialize(range, Status::Allocated);

            // Safety: `allocatable_run` came from the free list, which means it's an in-use
            // run and therefore part of `runs`, not `unused_runs`.
            let cursor = unsafe { self.runs.cursor_mut_from_ptr(allocatable_run) };
            drop(allocatable_run);
            drop(free_cursor);

            Self::add_run(allocated_run, cursor, &mut self.tracking);

            Ok(range)
        }
    }

    /// Return an object that implements [`Display`] to render the allocator's
    /// internal state.
    fn display_state(&self) -> impl fmt::Display + '_ {
        DisplayAllocatorState { allocator: self }
    }

    /// Adds `range` to the allocator as usable memory
    ///
    /// # Safety
    /// The caller must ensure that `range` is usable RAM and not already in use
    /// for another purpose.
    unsafe fn add_allocatable_range(&mut self, range: PageFrameRange) {
        let run = match self.tracking.unused_runs.pop_front() {
            Some(run) => run,
            None => todo!("allocate a new set of tracking pages"), /* will need to pass in a
                                                                    * MemoryAccess for this */
        };

        let mut state = run.inner.borrow_mut();
        state.range = range;
        state.status = Status::Free;
        drop(state);

        let cursor = Self::find_next(&mut self.runs, &run);
        Self::add_run(run, cursor, &mut self.tracking);
    }

    /// Search through the ordered list `list` for the next run after `run`
    fn find_next<'a>(list: &'a mut LinkedList<RunAdapter>, run: &Run) -> CursorMut<'a, RunAdapter> {
        let mut cursor = list.front_mut();
        while matches!(cursor.get(), Some(r) if r.start() < run.start()) {
            cursor.move_next();
        }
        cursor
    }

    /// Inserts a newly-created [`Run`] into the allocation lists. `cursor` must
    /// point to the next run in the list after `run`. Use `find_next` if that
    /// location is not already known.
    fn add_run(
        run: UnsafeRef<Run>,
        mut cursor: CursorMut<'_, RunAdapter>,
        tracking: &mut AllocatorTracking,
    ) {
        assert!(!run.link.is_linked());
        assert!(!run.free_link.is_linked());

        if run.status() == Status::Free {
            tracking.free.push_back(run.clone());
        }

        // Runs cannot overlap
        if let Some(next) = cursor.get() {
            assert!(next.start() >= run.end(), "{} and {} overlap", next, &*run);
        } else {
            // If the cursor is "null", then the current run must be going at
            // the end of the list
            let last_cursor = cursor.peek_prev();
            if let Some(last) = last_cursor.get() {
                assert!(last.end() <= run.start(), "{} and {} overlap", last, &*run);
            }
        }
        if let Some(prev) = cursor.peek_prev().get() {
            assert!(prev.end() <= run.start(), "{} and {} overlap", prev, &*run);
        }

        cursor.insert_before(run);
        // Coalesce the just-inserted node
        cursor.move_prev();
        // TODO: should probably only coalesce tracking+free runs? Otherwise freeing
        // allocations could get hairy? Although... coalescing allocated runs saves
        // bookkeeping memory
        Self::coalesce(cursor, tracking);
    }

    /// Coalesce the run pointed to by `cursor` with its neighbors, if possible.
    fn coalesce(mut cursor: CursorMut<'_, RunAdapter>, tracking: &mut AllocatorTracking) {
        if let Some(current) = cursor.get() {
            let can_coalesce_next = match cursor.peek_next().get() {
                Some(next) => next.status() == current.status() && next.start() == current.end(),
                None => false,
            };
            if can_coalesce_next {
                let size = current.size();

                // Handle the free list first because removing invalidates &current
                if current.status() == Status::Free {
                    debug_assert!(current.free_link.is_linked());
                    // Safety: if current is free, it must be in the free list (and we
                    // double-check above)
                    unsafe {
                        tracking.free.cursor_mut_from_ptr(current).remove();
                    }
                }

                let ptr = cursor.remove().unwrap();

                // Cursor now points to next
                cursor.get().unwrap().extend_left(size);

                tracking.unused_runs.push_back(ptr);
            }
        }

        // Refresh `current` since we may have coalesced it away
        if let Some(current) = cursor.get() {
            let can_coalesce_prev = match cursor.peek_prev().get() {
                Some(prev) => prev.status() == current.status() && prev.end() == current.start(),
                None => false,
            };
            if can_coalesce_prev {
                let size = current.size();

                // Handle the free list first because removing invalidates &current
                if current.status() == Status::Free {
                    debug_assert!(current.free_link.is_linked());
                    // Safety: if current is free, it must be in the free list (and we
                    // double-check above)
                    unsafe {
                        tracking.free.cursor_mut_from_ptr(current).remove();
                    }
                }

                let ptr = cursor.remove().unwrap();

                // .remove() moves to the _next_ element, so go back to get to
                // prev
                cursor.move_prev();
                cursor.get().unwrap().extend_right(size);

                tracking.unused_runs.push_back(ptr);
            }
        }
    }

    /// Initializes the memory pointed to by `range` as tracking memory. Empty
    /// runs are created and added into `unused_runs` for later access
    ///
    /// # Safety
    /// `range` must refer to usable RAM that is not already mapped or in use
    /// for another purpose.
    unsafe fn init_tracking_space(
        &mut self,
        access: &mut MemoryAccess,
        range: PageFrameRange,
    ) -> Result<(), Error> {
        let run_count = range.size_bytes() / mem::size_of::<Run>();
        defmt::debug!("Allocating {} runs in {}", run_count, range);

        // Ensure that changes to padding don't cause issues - currently, Rust doesn't
        // put padding between array elements, but if that changes, then the calculation
        // above will be wrong. So, validate that against the array size Rust thinks we
        // should need.
        assert!(
            Layout::array::<Run>(run_count)
                .map_err(|_| Error::new(ErrorKind::AddressOutOfBounds))?
                .size()
                <= range.size_bytes()
        );

        let mut ptr = access.map_permanent(range)?.cast::<MaybeUninit<Run>>();

        for _ in 0..run_count {
            (*ptr).write(Run {
                free_link: LinkedListLink::new(),
                link: LinkedListLink::new(),
                inner: RefCell::new(RunState {
                    range: PageFrameRange::empty(),
                    status: Status::Unused,
                }),
            });

            // The safety requirements of UnsafeRef are upheld because:
            // - this memory is permanantly allocated and marked as tracking
            // - it will only ever be accessed via the list it's inserted into
            let entry = UnsafeRef::from_raw((*ptr).as_ptr());
            self.tracking.unused_runs.push_back(entry);
            ptr = ptr.add(1);
        }

        let tracking_run = self.tracking.unused_runs.pop_front().unwrap(); // We just added a bunch of unused runs
        tracking_run.initialize(range, Status::Tracking);
        let cursor = Self::find_next(&mut self.runs, &tracking_run);
        Self::add_run(tracking_run, cursor, &mut self.tracking);

        Ok(())
    }
}

impl Run {
    fn status(&self) -> Status {
        self.inner.borrow().status
    }

    fn start(&self) -> PageFrame {
        self.inner.borrow().range.start()
    }

    fn end(&self) -> PageFrame {
        self.inner.borrow().range.end()
    }

    fn size(&self) -> usize {
        self.inner.borrow().range.size()
    }

    fn extend_left(&self, amount: usize) {
        self.inner.borrow_mut().range.extend_left(amount);
    }

    fn extend_right(&self, amount: usize) {
        self.inner.borrow_mut().range.extend_right(amount);
    }

    fn initialize(&self, range: PageFrameRange, status: Status) {
        let mut inner = self.inner.borrow_mut();
        debug_assert!(
            inner.status == Status::Unused,
            "Tried to reinitialize in-use run {}",
            inner
        );
        inner.status = status;
        inner.range = range;
    }
}

impl fmt::Display for Run {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.inner.try_borrow() {
            Ok(inner) => inner.fmt(f),
            Err(_) => write!(f, "<run in use>"),
        }
    }
}

impl fmt::Display for RunState {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} - {} ({} page frames) {:?}",
            self.range.start(),
            self.range.end(),
            self.range.size(),
            self.status
        )
    }
}

impl<'a> fmt::Display for DisplayAllocatorState<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for run in self.allocator.runs.iter() {
            writeln!(f, "* {}", run)?;
            if run.status() == Status::Free {
                // Safety: free nodes are always in the free list
                debug_assert!(run.free_link.is_linked());
                let cursor = unsafe { self.allocator.tracking.free.cursor_from_ptr(run) };
                write!(f, "    previous free: ")?;
                match cursor.peek_prev().get() {
                    Some(r) => writeln!(f, "{} - {}", r.start(), r.end())?,
                    None => writeln!(f, "none")?,
                }
                write!(f, "    next free: ")?;
                match cursor.peek_next().get() {
                    Some(r) => writeln!(f, "{} - {}", r.start(), r.end())?,
                    None => writeln!(f, "none")?,
                }
            }
        }
        writeln!(
            f,
            "{} unused runs",
            self.allocator.tracking.unused_runs.iter().count()
        )?;

        Ok(())
    }
}
