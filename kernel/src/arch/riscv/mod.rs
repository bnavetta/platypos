global_asm!(include_str!("boot.S"));

pub mod address;
pub mod sync;

use self::address::PhysicalAddress;

extern "C" {
    // Note: the way linker-provided symbols end up in Rust is a little odd - the
    // symbol is _at_ the provided address
    static _kernel_start: *const u8;
    static _kernel_end: *const u8;
}

pub fn kernel_start() -> PhysicalAddress {
    // Safety: _kernel_start is a symbol produced by the linker that's effectively
    // constant
    PhysicalAddress::new(unsafe { &_kernel_start as *const _ as usize })
}

pub fn kernel_end() -> PhysicalAddress {
    // Safety: _kernel_end is a symbol produced by the linker that's effectively
    // constant
    PhysicalAddress::new(unsafe { &_kernel_end as *const _ as usize })
}

pub fn abort() -> ! {
    loop {
        unsafe {
            riscv::asm::wfi();
        }
    }
}
