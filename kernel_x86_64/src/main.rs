#![no_std]
#![no_main]

// Needed even in 2018 edition since this crate isn't directly referenced
extern crate rlibc;

use core::panic::PanicInfo;

use log::{error, warn, info, debug};

use platypos_kernel::kernel_main;
use platypos_pal::mem::PageFrameRange;
use platypos_pal::mem::map::{MemoryMap, MemoryRegion};
use platypos_boot_info::BootInfo;
use x86_64_ext::instructions::hlt_loop;

mod logger;
mod mem;
mod platform;
mod conversions;

use crate::logger::KernelLog;
use crate::platform::Platform;
use crate::conversions::IntoPal;

static LOG: KernelLog = KernelLog::new();

#[export_name = "_start"]
extern "C" fn start(boot_info: &'static BootInfo) {
    if LOG.init().is_err() {
        // Can't even panic, since that logs
        hlt_loop()
    }
    debug!("Logging initialized");

    let memory_map = build_memory_map(boot_info);
    info!("Physical memory map:");
    for region in memory_map.iter() {
        info!("- {}", region)
    }

    kernel_main::<Platform>();

    // We don't expect to get here generally, but loop to be safe
    warn!("Returned from kernel_main");
    hlt_loop()
}

fn build_memory_map(boot_info: &BootInfo) -> MemoryMap<Platform> {
    boot_info.memory_map().map(|region| {
        let range = PageFrameRange::new(region.start.into_pal(), region.frames);
        MemoryRegion::new(range, region.flags)
    }).collect()
}

#[panic_handler]
fn handle_panic(info: &PanicInfo) -> ! {
    error!("Kernel panic! {}", info);
    hlt_loop();
}