use x86_64::instructions::port::Port;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExitCode {
    Success = 0x10,
    Failure = 0x11,
}

pub fn exit(code: ExitCode) -> ! {
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(code as u32);
    }

    // Not necessary, but makes the compiler happy
    loop {
        x86_64::instructions::hlt();
    }
}