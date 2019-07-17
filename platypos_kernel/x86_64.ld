ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS {
    /* . = 0x200000; */ /* 2Mib */
    . = 0x40000000; /* 1 GiB */
    /* I have a hunch that the bootloader doesn't like higher-half kernels because of the setup it does in 32-bit mode, but IDK */
    /* . = 0xffffffff80000000; */

    .rodata ALIGN(4K) : {
        *(.rodata*)
    }

    .eh_frame ALIGN(4K) : {
        *(.eh_frame*)
    }

    .text ALIGN(4K) : {
        *(.text*)
    }

    .data ALIGN(4K) : {
        *(.data*)
    }

    .bss ALIGN(4K) : {
        *(.bss*)
    }
}