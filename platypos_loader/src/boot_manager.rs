//! OS Boot Manager
//!
//! The `BootManager` represents the multi-stage process of loading and booting PlatypOS using the
//! typestate pattern.
//!
//! Booting consists of the following stages:
//!
//! - Stage 0: Creating the `BootManager`
//! - Stage 1: Mapping the UEFI address space. In order to context switch from the loader to the
//!   kernel, both must be in the same address space. We can rely on the fact that UEFI uses
//!   identity mapping and the kernel is in high memory, so the two do not overlap. To keep things
//!   simple, we use 1 GiB huge pages for the identity mapping. That means discarding granular
//!   page permissions, which is... a bit sketchy, but it's probably fine for now 🤞
//! - Stage 2: Loading the kernel image. This stage uses UEFI boot services to read the kernel ELF
//!   image from disk and map it into the boot address space.
//! - Stage 3: Exiting UEFI boot services. We have to exit boot services at some point and create
//!   the final memory map. Note that we can't allocate memory or log after this, as doing those
//!   relies on boot services.
//! - Stage 4: Handoff! Now that everything's ready, we can jump into the kernel!

use uefi::prelude::*;
use uefi::table::boot::MemoryType;
use uefi::table::SystemTableView;
use uefi::Handle;

use x86_64::structures::paging::PageTable;
use x86_64::PhysAddr;

// Stages
mod exit_uefi;
mod handoff;
mod load_kernel;
mod map_uefi;

mod util;

pub use load_kernel::LoadKernel;
pub use map_uefi::MapUefi;

/// Memory type for the kernel image (code and data)
pub const KERNEL_IMAGE: MemoryType = MemoryType(0x7000_0042);

// Memory type for data allocated by the OS loader for the kernel, such as
// the stack and boot information
pub const KERNEL_DATA: MemoryType = MemoryType(0x7000_0043);

/// Memory type for the initial kernel page table created by the OS loader
pub const KERNEL_PAGE_TABLE: MemoryType = MemoryType(0x7000_0044);

// Virtual address range for the kernel stack
pub const KERNEL_STACK_LOW: u64 = 0xffff_ffff_7100_0000;
pub const KERNEL_STACK_HIGH: u64 = 0xffff_ffff_7100_1000;

pub trait Stage {
    type SystemTableView: SystemTableView;
}

pub struct BootManager<S: Stage> {
    stage: S,
    image_handle: Handle,
    system_table: SystemTable<S::SystemTableView>,

    // Top-level page table (PML4) and its physical address
    page_table: &'static mut PageTable,
    page_table_address: PhysAddr,
}
