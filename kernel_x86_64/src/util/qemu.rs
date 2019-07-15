use x86_64::instructions::port::Port;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Success exit only used for tests
pub enum ExitCode {
    Success = 0x10,
    Failure = 0x11,
}

pub fn exit(code: ExitCode) -> ! {
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(code as u32);
    }

    unreachable!("QEMU did not exit");
}
