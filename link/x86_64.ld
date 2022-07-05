/* Linker script for x86-64 */

OUTPUT_FORMAT("elf64-x86-64", "elf64-x86-64", "elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)
ENTRY(_start)

# Copied from https://github.com/Amanieu/mini-backtrace/blob/main/eh_frame.ld
__eh_frame_hdr_start = ADDR(.eh_frame_hdr);
__eh_frame_hdr_end = ADDR(.eh_frame_hdr) + SIZEOF(.eh_frame_hdr);
__eh_frame_start = ADDR(.eh_frame);
__eh_frame_end = ADDR(.eh_frame) + SIZEOF(.eh_frame);

/* Adapted from https://github.com/knurling-rs/defmt/blob/7886fa030b133234db39aba5ce976e97155eb8f0/defmt/defmt.x.in */
EXTERN(_defmt_acquire);
EXTERN(_defmt_release);
EXTERN(__defmt_default_timestamp);
EXTERN(__DEFMT_MARKER_TIMESTAMP_WAS_DEFINED);
PROVIDE(_defmt_timestamp = __defmt_default_timestamp);
PROVIDE(_defmt_panic = __defmt_default_panic);

SECTIONS {
    /* Higher-half kernel in the upper 2GiB of the address space */
    . = 0xffffffff80000000;

    /*
     Adding the default defmt linker script on its own causes the bootloader to panic about already-mapped sections.
     I haven't exhaustively looked into this, but I _think_ that its SECTIONS entry causes the default SECTIONS to get
     skipped, so the linker no longer ensures that sections are page-aligned.
     */

    .text : ALIGN(4K) {
        *(.text .text.*)
    }

    .rodata : ALIGN(4K) {
        *(.rodata .rodata.*)
    }

    .data : ALIGN(4K) {
        *(.data .data.*)
    }

    .bss : ALIGN(4K) {
        *(.bss .bss.*)
    }

    .eh_frame_hdr : ALIGN(4K) {
        *(.eh_frame_hdr)
        *(.eh_frame_entry .eh_frame_entry.*)
    }

    .eh_frame : ALIGN(4K) {
        KEEP (*(.eh_frame)) *(.eh_frame.*)
    }

    /* DWARF debug sections */
    /* Adapted from /usr/lib/ldscripts/elf_x86_64.x */
    .debug 0 : { *(.debug) }
    .line 0 : { *(.line) }
    .debug_aranges 0 : { *(.debug_aranges) }
    .debug_pubnames 0 : { *(.debug_pubnames) }
    .debug_info 0 : { *(.debug_info) }
    .debug_abbrev 0 : { *(.debug_abbrev) }
    .debug_line 0 : { *(.debug_line .debug_line.* .debug_line_end) }
      .debug_frame    0 : { *(.debug_frame) }
  .debug_str      0 : { *(.debug_str) }
  .debug_loc      0 : { *(.debug_loc) }
  .debug_macinfo  0 : { *(.debug_macinfo) }
  /* SGI/MIPS DWARF 2 extensions.  */
  .debug_weaknames 0 : { *(.debug_weaknames) }
  .debug_funcnames 0 : { *(.debug_funcnames) }
  .debug_typenames 0 : { *(.debug_typenames) }
  .debug_varnames  0 : { *(.debug_varnames) }
  /* DWARF 3.  */
  .debug_pubtypes 0 : { *(.debug_pubtypes) }
  .debug_ranges   0 : { *(.debug_ranges) }
  /* DWARF 5.  */
  .debug_addr     0 : { *(.debug_addr) }
  .debug_line_str 0 : { *(.debug_line_str) }
  .debug_loclists 0 : { *(.debug_loclists) }
  .debug_macro    0 : { *(.debug_macro) }
  .debug_names    0 : { *(.debug_names) }
  .debug_rnglists 0 : { *(.debug_rnglists) }
  .debug_str_offsets 0 : { *(.debug_str_offsets) }
  .debug_sup      0 : { *(.debug_sup) }

    /* Adapted from https://github.com/knurling-rs/defmt/blob/7886fa030b133234db39aba5ce976e97155eb8f0/defmt/defmt.x.in */
    .defmt 1 (INFO) : ALIGN(4K) {
        . = 1;

        /* Format implementations for primitives like u8 */
        *(.defmt.prim.*);

        /* Everything user-defined */
        *(.defmt.*);

        __DEFMT_MARKER_END = .;

        /* Symbols that aren't referenced by the program and */
        /* should be placed at the end of the section */
        KEEP(*(.defmt.end .defmt.end.*));
    }
}

ASSERT(__DEFMT_MARKER_END < 65534, ".defmt section cannot contain more than 65534 interned strings");