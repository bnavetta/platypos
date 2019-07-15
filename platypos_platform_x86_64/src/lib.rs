#![no_std]

use x86_64::instructions::hlt;

use platypos_pal as pal;

pub struct Platform;

impl pal::Platform for Platform {
    const PAGE_SIZE: usize = 4096;

    fn halt() -> ! {
        loop {
            hlt()
        }
    }
}