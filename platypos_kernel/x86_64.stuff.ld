ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

/*
 * Like Linux, put the kernel in the upper 2GB of the address space
 * See https://read.seas.harvard.edu/cs161-18/doc/memory-layout/ and https://forum.osdev.org/viewtopic.php?f=1&t=31537 for the rationale
 * Linker script based on https://github.com/eteran/os64/blob/master/link/kernel.ld
 */

PHDRS {
    text PT_LOAD;
    rodata PT_LOAD;
    data PT_LOAD;
}

KERNEL_LMA = 0x0000000000400000; /* The bootloader puts the kernel at 4MiB */
KERNEL_VMA = 0xffffffff80000000 + KERNEL_LMA;

SECTIONS {

    . = KERNEL_VMA;

    KERNEL_START = .; /* Add a symbol to record the start address of the kernel */

    .text : AT(ADDR(.text) - KERNEL_VMA) {
        *(.text*)
    } :text

    . = ALIGN(4K);

    .rodata : AT(ADDR(.rodata) - KERNEL_VMA) {
        *(.rodata*)
    } :rodata

    . = ALIGN(4K);

    .data : AT(ADDR(.data) - KERNEL_VMA) {
        *(.data*)
    } :data

    . = ALIGN(4K);

    .bss : AT(ADDR(.bss) - KERNEL_VMA) {
        *(.bss*)
    } :data

    . = ALIGN(4K);

    .perprocessor : AT(ADDR(.perprocessor) - KERNEL_VMA) {
        PERPROCESSOR_START = . ;
        KEEP(*(.perprocessor))
        PERPROCESSOR_END = . ;
    } :data

    KERNEL_END = .; /* Add a symbol to record the end address of the kernel */
}
