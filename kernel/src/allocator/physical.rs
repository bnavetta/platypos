//! Physical memory allocator. This allocator supports power-of-two page
//! allocations (e.g. 1 page, 2 pages, 4 pages, etc.).
//!
//! The allocator is based on [Linux's buddy allocator](https://www.kernel.org/doc/gorman/html/understand/understand009.html).

use bitvec::prelude::*;
use intrusive_collections::{intrusive_adapter, LinkedList, LinkedListLink};

use crate::arch::address::{PhysicalAddress, PhysicalPage};

pub struct Allocator {
    free_areas: [FreeArea; Order::NUM_ORDERS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Order(u8);

/// An area containing blocks of free physical memory of a given order.
struct FreeArea {
    order: Order,

    /// Bitmap for tracking buddy state. Like Linux, this uses a single bit per
    /// pair of buddies - a `true` value means that either both are free or
    /// both are allocated, and a `false` value means that one is free and the
    /// other is allocated.
    buddy_map: &'static mut BitSlice,

    ///
    free_list: LinkedList<FreePageAdapter>,
}

struct FreePage {
    magic: usize,
    link: LinkedListLink,
    page: PhysicalPage,
}

intrusive_adapter!(FreePageAdapter = &'static FreePage : FreePage { link: LinkedListLink });

impl Allocator {
    /// Allocates `order.pages()` contiguous pages of physical memory, returning
    /// the starting page of the region. If there is insufficient free
    /// physical memory to satisfy the allocation, returns `None`.
    pub fn allocate_order(&mut self, order: Order) -> Option<PhysicalPage> {
        if let Some(free_page) = self.free_areas[order.index()].free_list.pop_front() {
            free_page.verify();
            self.toggle_buddy_state(free_page.page, order);
            Some(free_page.page)
        } else {
            let parent_order = order.parent()?;
            let parent_block = self.allocate_order(parent_order)?;

            let buddy = parent_block + order.pages();
            self.toggle_buddy_state(buddy, order);
            // Safety: buddy is the upper half of a just-allocated parent block, so it must
            // be free
            let buddy_page = unsafe { FreePage::from_page(buddy) };
            self.free_areas[order.index()]
                .free_list
                .push_front(buddy_page);
            Some(parent_block)
        }
    }

    pub fn free_order(&mut self, start_page: PhysicalPage, order: Order) {
        // Since we're freeing this block, if the new buddy state is `true` then both
        // buddies are free If the buddy is allocated, then the state goes from
        // `true` to `false`
        let both_free = self.toggle_buddy_state(start_page, order);
        let parent_order = order.parent();
        match (both_free, parent_order) {
            (true, Some(parent_order)) => {
                // In this case, we can coalesce with the buddy. To do so, we pull the buddy out
                // of its free list (otherwise it could be double-allocated) and
                // free the parent block.

                // Safety: based on the bitmap, we know that the buddy is free
                let free_buddy = unsafe { FreePage::from_free_page(order.buddy(start_page)) };
                // See https://github.com/Amanieu/intrusive-rs/issues/52
                let mut cursor = unsafe {
                    self.free_areas[order.index()]
                        .free_list
                        .cursor_mut_from_ptr(free_buddy)
                };
                cursor.remove();

                self.free_order(start_page, parent_order);
            }
            _ => {
                // In this case, we can't coalesce - either the buddy isn't free or there's no
                // higher order to coalesce into

                // Safety: we were just told to free this block
                let free_page = unsafe { FreePage::from_page(start_page) };
                self.free_areas[order.index()]
                    .free_list
                    .push_back(free_page);
            }
        }
    }

    /// Toggles the buddy state for a given block and returns the new state.
    /// - `true` if either both buddies are free or both are allocated
    /// - `false` if one is free and the other is allocated
    fn toggle_buddy_state(&mut self, page: PhysicalPage, order: Order) -> bool {
        // The buddy bit for a given PPN is at:
        //    PPN / (2 * order.pages())
        // Equivalently:
        //    PPN << (1 + order)
        // Rust is better able to optimize the bit-shifting version:
        // https://godbolt.org/#g:!((g:!((g:!((h:codeEditor,i:(filename:'1',fontScale:14,fontUsePx:'0',j:1,lang:rust,selection:(endColumn:1,endLineNumber:12,positionColumn:1,positionLineNumber:12,selectionStartColumn:1,selectionStartLineNumber:12,startColumn:1,startLineNumber:12),source:'//+Type+your+code+here,+or+load+an+example.%0Apub+fn+with_div(ppn:+usize,+order:+u8)+-%3E+usize+%7B%0A++++ppn+/+(2+*+(1+%3C%3C+order))%0A%7D%0A%0Apub+fn+with_shift(ppn:+usize,+order:+u8)+-%3E+usize+%7B%0A++++ppn+%3E%3E+(1+%2B+order)%0A%7D%0A%0A//+If+you+use+%60main()%60,+declare+it+as+%60pub%60+to+see+it+in+the+output:%0A//+pub+fn+main()+%7B+...+%7D%0A'),l:'5',n:'0',o:'Rust+source+%231',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0'),(g:!((h:compiler,i:(compiler:r1560,filters:(b:'0',binary:'1',commentOnly:'0',demangle:'0',directives:'0',execute:'1',intel:'0',libraryCode:'0',trim:'1'),flagsViewOpen:'1',fontScale:14,fontUsePx:'0',j:1,lang:rust,libs:!(),options:'-C++++++++++++++++opt-level%3D3',selection:(endColumn:1,endLineNumber:1,positionColumn:1,positionLineNumber:1,selectionStartColumn:1,selectionStartLineNumber:1,startColumn:1,startLineNumber:1),source:1,tree:'1'),l:'5',n:'0',o:'rustc+1.56.0+(Rust,+Editor+%231,+Compiler+%231)',t:'0')),k:50,l:'4',n:'0',o:'',s:0,t:'0')),l:'2',n:'0',o:'',t:'0')),version:4
        let map = &mut self.free_areas[order.index()].buddy_map;
        let index = page.page_number() << (1 + order.index());
        let new_state = !map[index];
        map.set(index, new_state);
        new_state
    }
}

impl Order {
    const ORDER_4KIB: Order = Order(0);
    const ORDER_8KIB: Order = Order(1);
    const ORDER_16KIB: Order = Order(2);
    const ORDER_32KIB: Order = Order(3);
    const ORDER_64KIB: Order = Order(4);
    const ORDER_128KIB: Order = Order(5);
    const ORDER_256KIB: Order = Order(6);
    const ORDER_512KIB: Order = Order(7);
    const ORDER_1MIB: Order = Order(8);
    const ORDER_2MIB: Order = Order(9);

    /// The number of supported orders.
    const NUM_ORDERS: usize = 10;

    const fn index(self) -> usize {
        self.0 as usize
    }

    /// The number of pages contained in a block of this order.
    const fn pages(self) -> usize {
        1 << self.0
    }

    /// The next order larger than this one, or `None` if this is the largest
    /// order.
    fn parent(self) -> Option<Order> {
        if self == Order::ORDER_2MIB {
            None
        } else {
            Some(Order(self.0 + 1))
        }
    }

    fn buddy(self, page: PhysicalPage) -> PhysicalPage {
        // A block and its buddy should only differ in the `order`th bit from the right
        let buddy_ppn = page.page_number() ^ (1 << self.index());
        PhysicalPage::from_page_number(buddy_ppn)
    }
}

impl FreePage {
    const MAGIC: usize = 0x112112321;

    fn verify(&self) {
        assert!(
            self.magic == FreePage::MAGIC,
            "Corrupted free-page magic {:#0x}",
            self.magic
        );
    }

    /// Gets the `FreePage` structure at `page`, assuming `page` is already free
    ///
    /// # Safety
    /// The caller must ensure that `page` is an already-free page
    unsafe fn from_free_page(page: PhysicalPage) -> &'static FreePage {
        let page_ptr = physical_to_mut_ptr(page.start_address()).cast::<FreePage>();
        let page_ref = page_ptr.as_ref().unwrap();
        page_ref.verify();
        assert!(
            page_ref.link.is_linked(),
            "Expected free page to be in free list"
        );
        assert!(
            page_ref.page == page,
            "Expected page data in free page to be {}",
            page
        );
        page_ref
    }

    /// Configures `page` as a free page.
    ///
    /// # Safety
    /// The caller must ensure that `page` refers to a page of physical free
    /// memory.
    unsafe fn from_page(page: PhysicalPage) -> &'static FreePage {
        let page_ptr = physical_to_mut_ptr(page.start_address()).cast::<FreePage>();
        (*page_ptr).magic = FreePage::MAGIC;
        (*page_ptr).link = LinkedListLink::new();
        (*page_ptr).page = page;
        page_ptr.as_ref().unwrap()
    }
}

// TODO: will need to reconsider when kernel enables virtual memory
pub fn physical_to_mut_ptr(addr: PhysicalAddress) -> *mut u8 {
    addr.as_usize() as *mut u8
}
