use alloc::vec;

use goblin::elf64::header::{self, Header};
use goblin::elf64::program_header::{self, ProgramHeader};

use log::{debug, info};

use plain::Plain;

use uefi::prelude::*;
use uefi::proto::media::file::{File, FileAttribute, FileMode, FileType, RegularFile};
use uefi::proto::media::fs::SimpleFileSystem;
use uefi::table::boot::AllocateType;

use x86_64::structures::paging::frame::PhysFrame;
use x86_64::structures::paging::{Page, PageTableFlags, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

use x86_64_ext::*;

use crate::memory_map::KERNEL_IMAGE;
use crate::page_table::KernelPageTable;

const KERNEL_FILENAME: &str = "platypos_kernel";

/// Representation of the kernel ELF image
pub struct KernelImage {
    header: Header,
    file: RegularFile,
}

impl KernelImage {
    pub fn entry_address(&self) -> VirtAddr {
        VirtAddr::new(self.header.e_entry)
    }
}

impl KernelImage {
    /// Opens the kernel image file
    pub fn open(boot_services: &BootServices) -> KernelImage {
        let mut kernel_file = locate_kernel(boot_services);

        kernel_file.set_position(0).unwrap_success();

        let mut buf = [0u8; header::SIZEOF_EHDR];
        assert_eq!(
            kernel_file
                .read(&mut buf)
                .expect_success("Could not read ELF header"),
            header::SIZEOF_EHDR
        );
        let header = Header::from_bytes(&buf);

        // Validate the header before returning
        assert_eq!(
            &header.e_ident[0..header::SELFMAG],
            header::ELFMAG,
            "Invalid ELF magic in kernel image"
        );
        assert_eq!(
            header.e_ident[header::EI_CLASS],
            header::ELFCLASS64,
            "Kernel image is not a 64-bit ELF file"
        );
        assert_eq!(
            header.e_machine,
            header::EM_X86_64,
            "Kernel not compiled for x86-64"
        );
        assert_eq!(
            header.e_type,
            header::ET_EXEC,
            "Kernel image is not an executable"
        );

        info!(
            "Kernel ELF header is valid. Entry point at {:#x}",
            header.e_entry
        );

        KernelImage {
            header: *header,
            file: kernel_file,
        }
    }

    /// Loads the kernel into memory
    pub fn load(&mut self, boot_services: &BootServices, page_table: &mut KernelPageTable) {
        self.file.set_position(self.header.e_phoff).unwrap_success();
        let mut buf = vec![0u8; self.header.e_phnum as usize * program_header::SIZEOF_PHDR];
        assert_eq!(
            self.file
                .read(&mut buf)
                .expect_success("Could not read program header table"),
            buf.len()
        );

        let headers = ProgramHeader::slice_from_bytes_len(&buf, self.header.e_phnum as usize)
            .expect("Could not parse program headers");

        for segment in headers.iter() {
            debug!(
                "{} segment at {:#x} ({} bytes)",
                program_header::pt_to_str(segment.p_type),
                segment.p_vaddr,
                segment.p_memsz
            );

            if segment.p_type == program_header::PT_LOAD {
                self.load_segment(boot_services, page_table, segment);
            }
        }
    }

    /// Loads and maps an individual segment of the kernel
    fn load_segment(
        &mut self,
        boot_services: &BootServices,
        page_table: &mut KernelPageTable,
        segment: &ProgramHeader,
    ) {
        let num_pages = Size4KiB::pages_containing(segment.p_memsz as usize);
        let phys_start = boot_services
            .allocate_pages(AllocateType::AnyPages, KERNEL_IMAGE, num_pages)
            .expect_success("Could not allocate memory for kernel segment");

        let buf: &mut [u8] =
            unsafe { core::slice::from_raw_parts_mut(phys_start as *mut u8, num_pages * 4096) };

        let offset = segment.p_vaddr as usize % 4096; // start of the actual segment contents within the allocated buffer
        let data_size = segment.p_filesz as usize; // size of the actual segment contents

        // In case p_vaddr isn't page-aligned, zero out any leading space
        for e in &mut buf[0..offset] {
            *e = 0;
        }

        // And in case it doesn't fill up the entire rest of the page frames, zero out any trailing space
        for e in &mut buf[offset + data_size..] {
            *e = 0
        }

        self.file
            .set_position(segment.p_offset)
            .expect_success("Could not seek to start of segment");
        assert_eq!(
            self.file
                .read(&mut buf[offset..offset + data_size])
                .expect_success("Could not read segment"),
            data_size
        );

        let mut flags = PageTableFlags::PRESENT | PageTableFlags::GLOBAL;
        if segment.p_flags & program_header::PF_W != 0 {
            flags |= PageTableFlags::WRITABLE;
        }
        if segment.p_flags & program_header::PF_X == 0 {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        let pages = Page::<Size4KiB>::containing_address(VirtAddr::new(segment.p_vaddr))
            .range_to(num_pages);
        let frames = PhysFrame::<Size4KiB>::from_start_address(PhysAddr::new(phys_start))
            .unwrap()
            .range_to(num_pages);
        page_table.map(boot_services, pages, frames, flags);

        debug!(
            "Loaded {:#x} - {:#x} of kernel image starting at {:#x}",
            segment.p_vaddr,
            segment.p_vaddr + segment.p_memsz,
            phys_start
        );
    }
}

/// Opens a handle to the kernel ELF file
fn locate_kernel(boot_services: &BootServices) -> RegularFile {
    let fs = boot_services
        .locate_protocol::<SimpleFileSystem>()
        .unwrap_success();
    let fs = unsafe { &mut *fs.get() };

    let mut root_directory = fs.open_volume().unwrap_success();
    let handle = root_directory
        .open(KERNEL_FILENAME, FileMode::Read, FileAttribute::empty())
        .unwrap_success();

    if let FileType::Regular(file) = handle.into_type().unwrap_success() {
        file
    } else {
        panic!("Found a directory at expected kernel location");
    }
}
