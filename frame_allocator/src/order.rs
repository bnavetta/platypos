use crate::FRAME_SIZE;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Ord, PartialOrd)]
pub struct Order(u8);

impl Order {
    pub const MAX: Order = Order(11);
    pub const MAX_VAL: usize = 11;

    pub const MIN: Order = Order(0);

    /// Returns the number of frames in a block of this order
    pub const fn frames(&self) -> usize {
        1usize << self.0
    }

    /// Returns the number of bytes in a block of this order
    const fn bytes(&self) -> usize {
        self.frames() * FRAME_SIZE
    }

    /// Returns the maximum allowed index for a block of this order. This relies on assuming a
    /// one-page tree, where the order-11 bitmap occupies 1 byte
    pub const fn max_index(&self) -> usize {
        // See the properties for orders listed above
        1 << (Order::MAX_VAL - self.as_usize() + 3)
    }

    pub fn parent(&self) -> Order {
        debug_assert!(*self < Order::MAX);
        Order(self.0 + 1)
    }

    pub fn child(&self) -> Order {
        debug_assert!(self.0 > 0);
        Order(self.0 - 1)
    }

    pub const fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

impl From<u8> for Order {
    fn from(v: u8) -> Order {
        debug_assert!((v as usize) <= Order::MAX_VAL);
        Order(v)
    }
}

impl From<usize> for Order {
    fn from(v: usize) -> Order {
        debug_assert!(v <= Order::MAX_VAL);
        Order(v as u8)
    }
}

impl Into<usize> for Order {
    fn into(self) -> usize {
        self.0 as usize
    }
}