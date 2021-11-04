/*
 * Kernel linker script for RISC-V
 * Adapted from https://github.com/sgmarz/osblog/blob/master/risc_v/src/lds/virt.lds
 */

OUTPUT_ARCH( "riscv" )

/*
 * Use _start as the entry point, by convention.
 */
ENTRY( _start )

/*
 * Put the kernel where it's expected by the SBI implementation
 */
BASE_ADDRESS = 0x80200000;


/*
 * Configure sections of the kernel binary. We're using Rust, so we need a text section for code, rodata for read-only
 * global constants, data for initialized global variables, and bss for uninitialized global variables.
 */
SECTIONS
{
    . = BASE_ADDRESS;

    PROVIDE(_kernel_start = .);

    /*
     * The .text section contains kernel code. We also include some symbols to find the start and end of the section.
     */
    .text : ALIGN(4K) {
        PROVIDE(_text_start = .);

        /* Put initialization code first, then any other text sections */
        *(.text.init)
        *(.text .text.*)

        PROVIDE(_text_end = .);
    }

    /*
     * The global pointer allows the linker to position global variables relative to
     * the gp (global pointer) register
     */
    PROVIDE(_global_pointer = .);

    /*
     * Read-only data (global constants).
     */
    .rodata : ALIGN(4K) {
        PROVIDE(_rodata_start = .);
        *(.rodata .rodata.*)
        PROVIDE(_rodata_end = .);
    }

    /*
     * Global initialized variables.
     */
    .data : ALIGN(4K) {
        PROVIDE(_data_start = .);

        /* Some compilers put shorter, faster-to-load sections in .sdata */
        *(.sdata .sdata.*)
        *(.data .data.*)

        PROVIDE(_data_end = .);
    }

    /*
     * Global uninitialized variables, which we must zero out at startup.
     */
    .bss (NOLOAD) : ALIGN(4K) {
        PROVIDE(_bss_start = .);
        *(.sbss .sbss.*)
        *(.bss .bss.*)
        PROVIDE(_bss_end = .);
    }

    /*
     * Start the stack right after the kernel. We're using a 64KiB stack (0x10000). Remember, the stack grows down!
    /* PROVIDE(_stack_start = _bss_end); */
    /* PROVIDE(_stack_end = _stack_start + 0x10000); */
    PROVIDE(_stack_end = _bss_end + 0x10000);

    PROVIDE(_kernel_end = _stack_end);

    /* Not using a static heap, gonna be fancier */
}