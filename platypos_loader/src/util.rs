use alloc::string::String;
use core::char::{decode_utf16, REPLACEMENT_CHARACTER};

use uefi::data_types::CStr16;
use x86_64::instructions::hlt;

pub fn to_string(str: &CStr16) -> String {
    decode_utf16(str.to_u16_slice().iter().cloned())
        .map(|r| r.unwrap_or(REPLACEMENT_CHARACTER))
        .collect()
}

pub fn halt_loop() -> ! {
    loop {
        hlt();
    }
}