//! x86-64 PAL implementation

use core::fmt;

use platypos_pal as pal;

use crate::mem::MemoryModel;

#[derive(Copy, Clone, Eq, PartialEq)]
pub struct Platform {

}

impl pal::Platform for Platform {
    type MemoryModel = MemoryModel;
}


impl fmt::Debug for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("x86-64 Platform")
    }
}
