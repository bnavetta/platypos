//! ELF representation

use alloc::vec::Vec;
use core::fmt;
use core::{
    cmp::{max, min},
    convert::TryInto,
    ops::Range,
};

use goblin::elf64::{
    header::{Header, EI_CLASS, ELFCLASS64, ELFMAG, EM_X86_64, SELFMAG, SIZEOF_EHDR},
    program_header::{
        pt_to_str, ProgramHeader, PF_R, PF_W, PF_X, PT_DYNAMIC, PT_LOAD, SIZEOF_PHDR,
    },
};
use log::info;
use uefi::prelude::*;
use x86_64::{
    align_down,
    structures::paging::{Page, PageTableFlags, PhysFrame},
    VirtAddr,
};

use crate::{file::File, page_table::KernelPageTable, KERNEL_IMAGE, PAGE_SIZE, util::allocate_frames};

/// An ELF object (binary). In practice, this will only ever be the kernel executable.
pub struct Object {
    file: File,
    pub metadata: ObjectMetadata,
}

/// Metadata about an ELF binary, parsed from its headers. This only supports statically linked binaries for x86-64.
pub struct ObjectMetadata {
    header: Header,
    program_headers: Vec<ProgramHeader>,
}

impl Object {
    /// Create a new ELF object given its backing file. This will read in ELF headers, but not load the executable into memory.
    pub fn new(mut file: File) -> Object {
        let metadata = ObjectMetadata::from_file(&mut file);
        Object { file, metadata }
    }

    /// Loads this object's segments into memory and updates `page_table` with the corresponding virtual memory mappings.
    pub fn load_and_map(&mut self, system_table: &SystemTable<Boot>, page_table: &mut KernelPageTable) {
        let vaddr_range = self.metadata.virtual_range();
        let base_virt_addr = align_down(vaddr_range.start, PAGE_SIZE);
        let pages = ((vaddr_range.end - base_virt_addr + PAGE_SIZE - 1) / PAGE_SIZE) as usize;

        let (base_phys_addr, buf) = allocate_frames(system_table, pages, KERNEL_IMAGE);
        info!(
            "Allocated {} pages for the kernel image starting at {:#x}",
            pages, base_phys_addr
        );

        // The compiler _probably_ optimizes this
        for entry in buf.iter_mut() {
            *entry = 0;
        }

        for phdr in self.metadata
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
            let bytes_read = self.file
                .read(
                    phdr.p_offset as usize,
                    &mut buf[buffer_offset..][..phdr.p_filesz as usize],
                )
                .expect_success("Could not read kernel image into memory");
            if bytes_read != phdr.p_filesz as usize {
                panic!("Could not fully read segment");
            }

            let mut flags = PageTableFlags::PRESENT;
            if phdr.p_flags & PF_W != 0 {
                flags |= PageTableFlags::WRITABLE;
            }
            if phdr.p_flags & PF_X == 0 {
                flags |= PageTableFlags::NO_EXECUTE;
            }

            // Figure out which pages we need to map for this segment
            let page_start = Page::containing_address(VirtAddr::new(phdr.p_vaddr));
            let frame_start = PhysFrame::containing_address(base_phys_addr + buffer_offset);
            let padding = phdr.p_vaddr - page_start.start_address().as_u64();
            let count = (padding + phdr.p_memsz + PAGE_SIZE - 1) / PAGE_SIZE;
            page_table.map(system_table, page_start, frame_start, count as usize, flags);
        }
    }
}

impl ObjectMetadata {
    /// Reads ELF object metadata from a file.
    pub fn from_file(file: &mut File) -> ObjectMetadata {
        let header = file.read_as::<Header, SIZEOF_EHDR>(0);

        // Verify the ELF header
        if &header.e_ident[0..SELFMAG] != ELFMAG {
            let magic = u64::from_le_bytes(
                header.e_ident[0..8]
                    .try_into()
                    .expect("ELF ident too short"),
            );
            panic!("Bad ELF magic: {:0x}", magic);
        }

        if header.e_ident[EI_CLASS] != ELFCLASS64 || header.e_machine != EM_X86_64 {
            panic!("Not a 64-bit ELF binary");
        }

        // Read program headers
        let program_headers = file.read_vec_as::<ProgramHeader, SIZEOF_PHDR>(
            header.e_phoff as usize,
            header.e_phnum as usize,
        );

        // No relocations, no dynamic linking, just a nice, static binary.
        for header in program_headers.iter() {
            if header.p_type == PT_DYNAMIC {
                panic!("Dynamic binaries not supported");
            }
        }

        ObjectMetadata {
            header,
            program_headers,
        }
    }

    /// The virtual address range used by this object when loaded into memory.
    pub fn virtual_range(&self) -> Range<u64> {
        let mut low = u64::MAX;
        let mut high = 0u64;
        for pheader in self
            .program_headers
            .iter()
            .filter(|ph| ph.p_type == PT_LOAD)
        {
            low = min(low, pheader.p_vaddr);
            high = max(high, pheader.p_vaddr + pheader.p_memsz);
        }

        if low == u64::MAX || high == 0u64 {
            panic!("Object contained no PT_LOAD segments");
        }

        low..high
    }

    /// Program headers for this object.
    pub fn program_headers(&self) -> &[ProgramHeader] {
        &self.program_headers
    }

    pub fn entry(&self) -> VirtAddr {
        VirtAddr::new(self.header.e_entry)
    }
}

impl fmt::Debug for ObjectMetadata {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        // ProgramHeader has a Debug implementation, but it only compiles with std :/
        struct DebugProgramHeaders<'a>(&'a [ProgramHeader]);
        impl<'a> fmt::Debug for DebugProgramHeaders<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_list()
                    .entries(self.0.iter().map(|ph| DebugProgramHeader(ph)))
                    .finish()
            }
        }

        struct DebugProgramHeader<'a>(&'a ProgramHeader);
        impl<'a> fmt::Debug for DebugProgramHeader<'a> {
            fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.debug_struct("ProgramHeader")
                    .field("p_type", &pt_to_str(self.0.p_type))
                    .field("p_flags", &format_args!("0x{:x}", self.0.p_flags))
                    .field("p_offset", &format_args!("0x{:x}", self.0.p_offset))
                    .field("p_vaddr", &format_args!("0x{:x}", self.0.p_vaddr))
                    .field("p_paddr", &format_args!("0x{:x}", self.0.p_paddr))
                    .field("p_filesz", &format_args!("0x{:x}", self.0.p_filesz))
                    .field("p_memsz", &format_args!("0x{:x}", self.0.p_memsz))
                    .field("p_align", &self.0.p_align)
                    .finish()
            }
        }

        f.debug_struct("ObjectMetadata")
            .field("header", &self.header)
            .field(
                "program_headers",
                &DebugProgramHeaders(&self.program_headers),
            )
            .finish()
    }
}
