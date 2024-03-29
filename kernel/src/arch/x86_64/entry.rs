//! Entry point for x86_64 systems

use core::fmt;
use core::mem::MaybeUninit;

use {platypos_hal_x86_64 as hal_impl, platypos_ktrace as ktrace};

use bootloader_api::info::{MemoryRegion, MemoryRegionKind};
use bootloader_api::{entry_point, BootInfo, BootloaderConfig};

use crate::arch::mm::MemoryAccess;
use crate::mm::map::Region;
use crate::mm::{heap_allocator, root_allocator};
use crate::{trace, BootArgs};

use super::display::FrameBufferTarget;

pub static BOOTLOADER_CONFIG: BootloaderConfig = {
    let mut config = BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};

/// Entry point called by the bootloader
fn start(info: &'static mut BootInfo) -> ! {
    unsafe {
        heap_allocator::init();
    }

    let ic = hal_impl::interrupts::init();

    trace::init(
        unsafe { hal_impl::SerialPort::new(0x3f8) },
        &crate::arch::hal_impl::topology::INSTANCE,
        ic,
    );
    trace::flush();

    let _span = tracing::info_span!("start").entered();
    trace::flush();

    let version = info.api_version;
    tracing::info!(
        "Booting from bootloader v{}.{}.{}{}",
        version.version_major(),
        version.version_minor(),
        version.version_patch(),
        if version.pre_release() {
            " (prerelease)"
        } else {
            ""
        }
    );

    tracing::info!("Memory Regions:");
    trace::flush();
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
    trace::flush();

    tracing::info!("HERE!");
    trace::flush();

    let access = unsafe {
        MemoryAccess::init(
            info.physical_memory_offset.into_option().unwrap() as usize as *mut MaybeUninit<u8>
        )
    };
    trace::flush();

    // TODO: add kernel?
    let reserved = &[];

    tracing::debug!("Before allocator init");
    trace::flush();

    let root_allocator = root_allocator::init(
        &access,
        ic,
        info.memory_regions.iter().map(Region::from), /* TODO: end of failing region seems off,
                                                       * but also start isn't page-aligned?
                                                       * right after bootloader */
        reserved,
    )
    .expect("Root allocator initialization failed");
    trace::flush();

    tracing::debug!("After allocator init");
    trace::flush();

    heap_allocator::enable_expansion(root_allocator);

    // Initialize the local interrupt controller after setting up memory allocation,
    // in case there's any dynamic data
    hal_impl::interrupts::init_local();

    tracing::debug!("Platform-specific initialization complete, entering kmain");
    trace::flush();

    let args = BootArgs {
        display: info.framebuffer.as_mut().map(FrameBufferTarget::new),
        memory_access: access,
        root_allocator,
        interrupt_controller: ic,
    };

    crate::kmain(args);
}

fn log_region(region: MemoryRegion) {
    let size = region.end - region.start;
    tracing::info!(
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

entry_point!(start, config = &BOOTLOADER_CONFIG);

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
