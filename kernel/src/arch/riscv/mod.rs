global_asm!(include_str!("boot.S"));

pub mod sync;

pub fn abort() -> ! {
    loop {
        unsafe {
            riscv::asm::wfi();
        }
    }
}
