use bootloader::BootInfo;

pub mod alloc;
pub mod frame;
pub mod page_table;

pub fn init(info: &'static BootInfo) {
    self::page_table::init(info);
    self::frame::init(info);
}
