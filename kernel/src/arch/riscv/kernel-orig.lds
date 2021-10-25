/*
 * Kernel linker script for RISC-V
 * Adapted from https://github.com/sgmarz/osblog/blob/master/risc_v/src/lds/virt.lds
 */

OUTPUT_ARCH( "riscv" )

/*
 * Use _start as the entry point, by convention.
 * IIUC, it shouldn't matter exactly where this is because OpenSBI understands ELF files
 */
ENTRY( _start )

/*
 * Tell the linker we have a single block of r/w executable RAM. This is just for linking, so it doesn't need to
 * describe the firmware, MMIO regions, etc.
 */
MEMORY
{
    ram (wxa) : ORIGIN = 0x800000000, LENGTH = 128M
}

/*
 * Configure program headers, which apply at load-time and tell the firmware how to read the kernel into memory
 */
PHDRS
{
    text PT_LOAD;
    data PT_LOAD;
    bss PT_LOAD;
}

/*
 * Configure sections of the kernel binary. We're using Rust, so we need a text section for code, rodata for read-only
 * global constants, data for initialized global variables, and bss for uninitialized global variables.
 */
SECTIONS
{
    /*
     * The .text section contains kernel code. We also include some symbols to find the start and end of the section.
     */
    .text : {
        PROVIDE(_text_start = .);

        /* Put initialization code first, then any other text sections */
        *(.text.init)
        *(.text .text.*)

        PROVIDE(_text_end = .);
    } >ram AT>ram :text /* Put the .text section into RAM, identity-mapped */

    /*
     * The global pointer allows the linker to position global variables relative to
     * the gp (global pointer) register
     */
    PROVIDE(_global_pointer = .);

    /*
     * Read-only data (global constants).
     */
    .rodata : {
        PROVIDE(_rodata_start = .);
        *(.rodata .rodata.*)
        PROVIDE(_rodata_end = .);
    } >ram AT>ram :text /* Put this in the text section too, since it's also read-only */

    /*
     * Global initialized variables.
     */
    .data : {
        . = ALIGN(4096);
        PROVIDE(_data_start = .);

        /* Some compilers put shorter, faster-to-load sections in .sdata */
        *(.sdata .sdata.*)
        *(.data .data.*)

        PROVIDE(_data_end = .);
    } >ram AT>ram :data

    /*
     * Global uninitialized variables, which we must zero out at startup.
     */
    .bss : {
        PROVIDE(_bss_start = .);
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        PROVIDE(_bss_end = .);
    } >ram AT>ram :bss

    /*
     * Keep track of the RAM region and use it plus the kernel size to figure out a kernel stack.
     */
    PROVIDE(_memory_start = ORIGIN(ram));
    /*
     * Start the stack right after the kernel. We're using a 64KiB stack (0x10000). Remember, the stack grows down!
    /* PROVIDE(_stack_start = _bss_end); */
    /* PROVIDE(_stack_end = _stack_start + 0x10000); */
    PROVIDE(_stack_end = _bss_end + 0x10000);

    /* Not using a static heap, gonna be fancier */
}