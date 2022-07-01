//! Entry point for x86_64 systems

use core::fmt;
use core::mem::MaybeUninit;

use bootloader::boot_info::{MemoryRegion, MemoryRegionKind};
use bootloader::{entry_point, BootInfo};

use crate::arch::mm::MemoryAccess;
use crate::mm::map::Region;
use crate::prelude::*;
use crate::BootArgs;

use super::display::FrameBufferTarget;

/// Entry point called by the bootloader
fn start(info: &'static mut BootInfo) -> ! {
    let mut serial = unsafe { uart_16550::SerialPort::new(0x3f8) };
    serial.init();
    crate::logging::init(serial);

    log::info!(
        "Booting from bootloader v{}.{}.{}{}",
        info.version_major,
        info.version_minor,
        info.version_patch,
        if info.pre_release {
            " (prerelease)"
        } else {
            ""
        }
    );

    log::info!("Memory Regions:");
    // The bootloader doesn't combine adjacent functionally-equivalent regions, so
    // do it here
    // It also marks UEFI runtime service memory as usable...
    // TODO: accumulate into a vec
    let last = info.memory_regions.iter().cloned().reduce(|prev, next| {
        if prev.kind == next.kind && prev.end == next.start {
            // Combine!
            MemoryRegion {
                start: prev.start,
                end: next.end,
                kind: prev.kind,
            }
        } else {
            // Can't merge, report the previous region
            log_region(prev);
            next
        }
    });
    if let Some(last) = last {
        log_region(last);
    }

    log::info!("Allocator regions:");
    let mut access = unsafe {
        MemoryAccess::new(
            info.physical_memory_offset.into_option().unwrap() as usize as *mut MaybeUninit<u8>
        )
    };

    // TODO: add kernel?
    let reserved = &[];

    let mut ab = crate::mm::root_allocator::Builder;
    ab.parse_memory_map(
        &mut access,
        info.memory_regions.iter().map(Region::from),
        reserved,
    )
    .unwrap();

    let args = BootArgs {
        display: info.framebuffer.as_mut().map(FrameBufferTarget::new),
        memory_access: access,
    };

    crate::kmain(args);
}

fn log_region(region: MemoryRegion) {
    let size = region.end - region.start;
    log::info!(
        " - {:#012x} - {:#012x} {} ({} bytes =~ {} KiB =~ {} MiB)",
        region.start,
        region.end,
        DisplayRegion(region.kind),
        size,
        size / 1024,
        size / 1024 / 1024
    );
}

struct DisplayRegion(MemoryRegionKind);

impl fmt::Display for DisplayRegion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self.0 {
            MemoryRegionKind::UnknownBios(v) => write!(f, "Unknown BIOS type {v}"),
            MemoryRegionKind::UnknownUefi(v) => match v {
                // See https://uefi.org/specs/ACPI/6.4/15_System_Address_Map_Interfaces/uefi-getmemorymap-boot-services-function.html
                0 => write!(f, "UEFI Reserved"),
                1 => write!(f, "UEFI Loader (Code)"),
                2 => write!(f, "UEFI Loader (Data)"),
                3 => write!(f, "UEFI Boot Services (Code)"),
                4 => write!(f, "UEFI Boot Services (Data)"),
                5 => write!(f, "UEFI Runtime Services (Code)"),
                6 => write!(f, "UEFI Runtime Services (Data)"),
                7 => write!(f, "Conventional memory"),
                8 => write!(f, "Unusable"),
                9 => write!(f, "ACPI (reclaimable)"),
                10 => write!(f, "ACPI NVS"),
                11 => write!(f, "Memory-Mapped I/O"),
                12 => write!(f, "Memory-Mapped I/O Port Space"),
                13 => write!(f, "UEFI PAL Code"),
                14 => write!(f, "Persistent memory"),
                other => write!(f, "Unknown UEFI type {other}"),
            },
            other => write!(f, "{other:?}"),
        }
    }
}

entry_point!(start);

/*

Physical memory management
- in practice, just allocating pages is enough - don't bother with 2^n page allocations / buddies
- use a fixed-size stack of free frames for speed, plus a bitmap to hold remaining frame state
- frames that are on the stack are marked as allocated in the bitmap
- can mark memory holes as allocated in the bitmap also, and/or make the bitmap an array of regions (can abstract that away)
  - may depend on size - if there's a region too small to track on its own, combine with another but track the hole?
  - e.g. combine consecutive regions of <10 MiB and figure there may be holes to track
- if stack is empty, scan bitmap for free pages to refill it


Grouping algorithm:
  Assume that regions are sorted and non-overlapping

  let current_region = regions_iter.find(|r| r.kind is usable)
  for region in regions_iter: # Look at regions starting from the first usable one
    if current_region is None:
      current_region = Some(region)
    else if region.kind is usable or region.size < hole threshold:
      combine with current_region
    else:
      add current_region to allocation_zones
      current_region = Some(region)

  # Now, do another pass to mark holes:
  for region in regions_iter:
    if region.kind is not usable:
      containing_region = allocation_zones.binary_search(region)
      mark region as unallocatable in containing_region

  # And similarly mark other regions as unallocatable, like the kernel
  # to mark as unallocatable, can just permanently set bits to allocated, then they won't get used




Physical memory access
- abstract away whether physical memory is mapped into the kernel address space
- two APIs:
   - permanently map a chunk of physical memory (needed to create bitmaps for physical memory manager)
     - infinite recursion risk if this needs to allocate frames to create the virtual memory mappings
     - have parameter that specifies how to allocate page frames:
       - normally, ask physical memory manager for them
       - if you _are_ the physical memory manager, provide a preallocated buffer of frames
   - temporarily map a chunk of physical memory using a RAII guard
     - this could just use an existing mapping, or map it into a reserved chunk of address space
- platform_common should provide a default implementation for platforms where all physical memory is mapped

Other stuff:
- x86_64 bootloader crate (and probably other platforms) can pass TLS (thread-local storage) info to kernel
- see if that can be repurposed as CPU-local storage (need to figure out how it's accessed)
- use thingbuf to send info from interrupt handlers to regular (or high-priority even) tasks

*/
