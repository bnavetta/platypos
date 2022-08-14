mod entry;

pub mod display;
pub mod mm;

/// The base page size for this platform.
pub const PAGE_SIZE: usize = 4096;

// HAL bindings - other parts of the kernel need to know which HAL
// implementation they're using (mostly to put it in static vars)
pub use platypos_hal_x86_64 as hal_impl;
