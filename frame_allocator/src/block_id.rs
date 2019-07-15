use crate::order::Order;

/// A `BlockId` refers to a block of physical memory, contingent on the order (size) of that block.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct BlockId {
    order: Order,
    index: usize,
}

impl BlockId {
    pub fn new(order: Order, index: usize) -> BlockId {
        debug_assert!(index <= order.max_index());
        BlockId { order, index }
    }

    #[inline(always)]
    pub fn order(&self) -> Order {
        self.order
    }

    #[inline(always)]
    pub fn index(&self) -> usize {
        self.index
    }

    #[inline(always)]
    pub fn sibling(&self) -> BlockId {
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
    pub fn parent(&self) -> Option<BlockId> {
        if self.order < Order::MAX {
            let parent = (self.index & !1) >> 1;
            Some(BlockId::new(self.order.parent(), parent))
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn left_child(&self) -> BlockId {
        debug_assert!(self.order > Order::MIN);
        BlockId::new(self.order.child(), self.index << 1)
    }

    #[inline(always)]
    pub fn right_child(&self) -> BlockId {
        debug_assert!(self.order > Order::MIN);
        BlockId::new(self.order.child(), (self.index << 1) + 1)
    }
}
