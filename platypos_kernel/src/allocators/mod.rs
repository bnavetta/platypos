use core::alloc::Layout;

pub mod heap;
pub mod physical;

#[alloc_error_handler]
fn allocation_error(layout: Layout) -> ! {
    panic!("Unable to allocate {} bytes", layout.size());
}
