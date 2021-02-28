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
    program_header::{pt_to_str, ProgramHeader, PT_DYNAMIC, PT_LOAD, SIZEOF_PHDR},
};
use x86_64::VirtAddr;

use crate::file::File;

/// An ELF object. This is general-purpose-ish, but really only handles loading the kernel.
pub struct Object {
    header: Header,
    program_headers: Vec<ProgramHeader>,
}

impl Object {
    /// Reads ELF object metadata from a file.
    pub fn from_file(file: &mut File) -> Object {
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
        let program_headers = file.read_vec_as::<ProgramHeader, SIZEOF_PHDR>(header.e_phoff as usize, header.e_phnum as usize);

        // No relocations, no dynamic linking, just a nice, static binary.
        for header in program_headers.iter() {
            if header.p_type == PT_DYNAMIC {
                panic!("Dynamic binaries not supported");
            }
        }

        Object {
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

impl fmt::Debug for Object {
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

        f.debug_struct("Object")
            .field("header", &self.header)
            .field(
                "program_headers",
                &DebugProgramHeaders(&self.program_headers),
            )
            .finish()
    }
}
