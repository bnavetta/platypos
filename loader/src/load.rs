//! Loading the kernel image into memory

use core::slice;

use goblin::elf64::program_header::{PF_R, PF_W, PF_X, PT_LOAD};
use log::info;
use uefi::prelude::*;
use uefi::table::boot::AllocateType;
use x86_64::{
    align_down,
    structures::paging::{Page, PageTableFlags, PhysFrame},
    PhysAddr, VirtAddr,
};

use crate::elf::Object;
use crate::file::File;
use crate::page_table::KernelPageTable;
use crate::KERNEL_IMAGE;

const PAGE_SIZE: u64 = 4096;


pub fn load_kernel_image(
    system_table: &SystemTable<Boot>,
    kernel_image: &Object,
    kernel_file: &mut File,
    page_table: &mut KernelPageTable,
) {
    let vaddr_range = kernel_image.virtual_range();
    info!("Kernel is {} bytes", vaddr_range.end - vaddr_range.start);
    let base_virt_addr = align_down(vaddr_range.start, PAGE_SIZE);
    let pages = ((vaddr_range.end - base_virt_addr + PAGE_SIZE - 1) / PAGE_SIZE) as usize;

    let base_phys_addr = PhysAddr::new(
        system_table
            .boot_services()
            .allocate_pages(AllocateType::AnyPages, KERNEL_IMAGE, pages)
            .expect_success("Could not allocate memory for kernel image"),
    );
    info!(
        "Allocated {} pages for the kernel image starting at {:#x}",
        pages, base_phys_addr
    );

    // Safety: this address was just returned from the UEFI page allocator
    let buf = unsafe {
        slice::from_raw_parts_mut(
            base_phys_addr.as_u64() as *mut u8,
            pages * PAGE_SIZE as usize,
        )
    };
    // The compiler _probably_ optimizes this
    for entry in buf.iter_mut() {
        *entry = 0;
    }

    for phdr in kernel_image
        .program_headers()
        .iter()
        .filter(|ph| ph.p_type == PT_LOAD)
    {
        let perm = |flag: u32, symbol: char| {
            if phdr.p_flags & flag != 0 {
                symbol
            } else {
                '-'
            }
        };

        info!(
            "Loading segment {:#x} - {:#x} ({} bytes) [{}{}{}]",
            phdr.p_vaddr,
            phdr.p_vaddr + phdr.p_memsz,
            phdr.p_memsz,
            perm(PF_R, 'r'),
            perm(PF_W, 'w'),
            perm(PF_X, 'x')
        );
        let buffer_offset = (phdr.p_vaddr - base_virt_addr) as usize;
        let bytes_read = kernel_file
            .read(
                phdr.p_offset as usize,
                &mut buf[buffer_offset..][..phdr.p_filesz as usize],
            )
            .expect_success("Could not read kernel image into memory");
        if bytes_read != phdr.p_filesz as usize {
            panic!("Could not fully read segment");
        }

        let mut flags = PageTableFlags::GLOBAL | PageTableFlags::PRESENT;
        if phdr.p_flags & PF_W != 0 {
            flags |= PageTableFlags::WRITABLE;
        }
        if phdr.p_flags & PF_X == 0 {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        // Figure out which pages we need to map for this segment
        let page_start = Page::containing_address(VirtAddr::new(phdr.p_vaddr));
        let frame_start = PhysFrame::containing_address(base_phys_addr + buffer_offset);
        let count = (phdr.p_memsz + PAGE_SIZE - 1) / PAGE_SIZE;
        page_table.map(system_table, page_start, frame_start, count as usize, flags);
    }
}