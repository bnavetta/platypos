use alloc::vec::Vec;
use core::convert::{AsRef, TryFrom};

use uefi::{CStr16, Char16};

// An owned UCS-2 string
#[repr(transparent)]
#[derive(Debug, Eq)]
pub struct CString16(Vec<Char16>);

impl CString16 {
    pub fn from_str(s: &str) -> CString16 {
        let mut buf = Vec::with_capacity(s.len() / 2 + 1);
        buf.extend(s.encode_utf16().map(to_char));
        buf.push(to_char(0)); // needs trailing null byte
        CString16(buf)
    }
}

impl AsRef<CStr16> for CString16 {
    fn as_ref(&self) -> &CStr16 {
        // Lifetime bounds ensure that the returned reference does not live longer than this CString16
        unsafe { CStr16::from_ptr(self.0.as_ptr()) }
    }
}

impl PartialEq for CString16 {
    fn eq(&self, other: &CString16) -> bool {
        self.0 == other.0
    }
}

impl PartialEq<&CStr16> for CString16 {
    fn eq(&self, other: &&CStr16) -> bool {
        self.as_ref().to_u16_slice_with_nul() == other.to_u16_slice_with_nul()
    }
}

fn to_char(c: u16) -> Char16 {
    match Char16::try_from(c) {
        Ok(ch) => ch,
        Err(_) => panic!("Not a valid UTF-16 character: {}", c),
    }
}
