use core::alloc::Layout;

pub mod physical;
pub mod heap;

#[alloc_error_handler]
fn allocation_error(layout: Layout) -> ! {
    panic!("Unable to allocate {} bytes", layout.size());
}
