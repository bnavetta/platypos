use core::slice;

use goblin::elf64::header::{self, Header};
use goblin::elf64::program_header::{self, ProgramHeader};
use log::{debug, info};
use plain::Plain;
use uefi::prelude::*;
use uefi::proto::media::file::{FileType, RegularFile};
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;

use super::util::{make_page_range, make_frame_range};
use super::{BootManager, Stage, KERNEL_IMAGE, KERNEL_DATA, KERNEL_STACK_LOW, KERNEL_STACK_HIGH};
use crate::filesystem::locate_file;

pub struct LoadKernel;

impl Stage for LoadKernel {
    type SystemTableView = Boot;
}

impl BootManager<LoadKernel> {
    pub fn load_kernel(mut self) {
        let mut kernel_file = self.locate_kernel(&["platypos_kernel"]);

        let header = self.read_elf_header(&mut kernel_file);
        debug!("Kernel entry address is {:#x}", header.e_entry);

        // Read in the segment headers
        kernel_file.set_position(header.e_phoff).unwrap_success();
        let mut program_header_buf =
            vec![0u8; header.e_phnum as usize * program_header::SIZEOF_PHDR];
        assert_eq!(
            kernel_file
                .read(&mut program_header_buf)
                .expect_success("Could not read kernel program header table"),
            program_header_buf.len()
        );
        let program_headers =
            ProgramHeader::slice_from_bytes_len(&program_header_buf, header.e_phnum as usize)
                .expect("Could not parse kernel program header table");

        for segment in program_headers {
            debug!(
                "{} segment at {:#x} - {:#x} ({} bytes)",
                program_header::pt_to_str(segment.p_type),
                segment.p_vaddr,
                segment.p_vaddr + segment.p_memsz,
                segment.p_memsz
            );

            if segment.p_type == program_header::PT_LOAD {
                self.load_segment(&mut kernel_file, segment);
            }
        }

        self.allocate_kernel_stack();

        info!("Kernel image loaded");
    }

    fn locate_kernel(&self, kernel_path: &[&str]) -> RegularFile {
        let file = locate_file(self.system_table.boot_services(), kernel_path)
            .expect_success("Could not locate kernel")
            .expect("Kernel not found");

        if let FileType::Regular(file) = file.into_type().unwrap_success() {
            file
        } else {
            panic!("Found a directory at the expected kernel location")
        }
    }

    fn read_elf_header(&self, kernel_file: &mut RegularFile) -> Header {
        kernel_file.set_position(0).unwrap_success();

        let mut header_buf = [0u8; header::SIZEOF_EHDR];

        assert_eq!(
            kernel_file
                .read(&mut header_buf)
                .expect_success("Could not read ELF header"),
            header::SIZEOF_EHDR,
            "Could not read entire ELF header"
        );
        let header = Header::from_bytes(&header_buf);

        // Validate the header
        assert_eq!(
            &header.e_ident[0..header::SELFMAG],
            header::ELFMAG,
            "Invalid ELF magic"
        );
        assert_eq!(
            header.e_ident[header::EI_CLASS],
            header::ELFCLASS64,
            "Not a 64-bit ELF file"
        );
        assert_eq!(
            header.e_machine,
            header::EM_X86_64,
            "Kernel is not an x86-64 binary"
        );
        assert_eq!(
            header.e_type,
            header::ET_EXEC,
            "Kernel is not an executable"
        );
        debug!("Kernel header is valid");

        *header
    }

    fn load_segment(&mut self, kernel_file: &mut RegularFile, segment: &ProgramHeader) {
        let pages = (segment.p_memsz as usize + 4095) / 4096;
        let phys_addr = self
            .allocate_pages(KERNEL_IMAGE, pages)
            .expect("Failed to allocate memory for kernel segment");

        let buffer: &mut [u8] = unsafe {
            slice::from_raw_parts_mut(phys_addr.as_u64() as usize as *mut u8, pages * 4096)
        };
        // Ensure the buffer is zeroed out, especially since there might be additional space before or
        // after the on-disk data (i.e. if the segment isn't page-aligned or memsz > filesz)
        for e in buffer.iter_mut() {
            *e = 0;
        }

        let offset = segment.p_vaddr as usize % 4096; // Segment start isn't necessarily page-aligned
        kernel_file
            .set_position(segment.p_offset)
            .expect_success("Could not seek to segment");
        let on_disk_size = segment.p_filesz as usize;
        assert_eq!(
            kernel_file
                .read(&mut buffer[offset..offset + on_disk_size])
                .expect_success("Could not read segment"),
            on_disk_size
        );

        let mut flags = PageTableFlags::PRESENT;
        if segment.p_flags & program_header::PF_W != 0 {
            flags |= PageTableFlags::WRITABLE;
        }
        if segment.p_flags & program_header::PF_X == 0 {
            flags |= PageTableFlags::NO_EXECUTE;
        }

        self.map_contiguous_4kib(
            make_page_range(VirtAddr::new(segment.p_vaddr).align_down(4096u64), pages),
            make_frame_range(phys_addr, pages),
            flags
        );

        debug!(
            "Loaded segment {:#x} - {:#x} at physical address {:#x}",
            segment.p_vaddr,
            segment.p_vaddr + segment.p_memsz,
            phys_addr
        );
    }

    fn allocate_kernel_stack(&mut self) {
        assert_eq!((KERNEL_STACK_HIGH - KERNEL_STACK_LOW) % 4096, 0, "Kernel stack size is not an integer number of pages");

        let pages = (KERNEL_STACK_HIGH - KERNEL_STACK_LOW) as usize / 4096;
        let phys_addr = self.allocate_pages(KERNEL_DATA, pages).expect("Could not allocate kernel stack");

        self.map_contiguous_4kib(
            make_page_range(VirtAddr::new(KERNEL_STACK_LOW), pages),
            make_frame_range(phys_addr, pages),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE | PageTableFlags::NO_EXECUTE
        );

        // Zero out the stack
        let stack: *mut u8 = phys_addr.as_u64() as usize as *mut u8;
        unsafe {
            stack.write_bytes(0, pages * 4096);
        }

        debug!("Allocated kernel stack at {:#x} ({} bytes)", phys_addr, pages * 4096);
    }
}
