use x86_64::VirtAddr;

pub mod screen;

pub fn init() {
    self::screen::init(VirtAddr::new(0xb8000));
}