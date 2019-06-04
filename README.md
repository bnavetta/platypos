# PlatypOS

PlatypOS is a microkernel for the x86-64 architecture. It's very much a work in progress.

# Notes

This is basically scratch space for me.

## Kernel Address Space

Things I've put in the kernel address space (on top of the kernel code and physical memory mapping the bootloader puts
there).

* `0xfffffa0000000000-0xfffffa0000040000` - ACPI table mappings (only used during `topology::acpi::discover`)
* `0xfffffa0000040000-0xfffffa0000041000` - HPET registers
* `0xfffffb0000000000-0xfffffb0100000000` - Kernel heap
* `0xfffffbffffffc000-0xfffffbffffffe000` - fault-handling stack
* `0xfffffc0000000000-???`                - physical memory map