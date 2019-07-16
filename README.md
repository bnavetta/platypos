# PlatypOS

PlatypOS a very-WIP microkernel.

## Organization

PlatypOS is split across several crates:

* `platypos_config` - build-time configuration such as the maximum number of supported processors
* `platypos_test` - minimal no_std testing framework
* `platypos_kernel` - the kernel itself

Within the kernel, platform-specific code is in the `platform` module, which is conditionally compiled to use the
appropriate implementation for the target system.

# Notes

This is basically just scratch space for me.

## Kernel Address Space

Things I've put in the kernel address space (on top of the kernel code and physical memory mapping the bootloader puts
there).

* `0xfffffa0000000000-0xfffffa0000040000` - ACPI table mappings (only used during `topology::acpi::discover`)
* `0xfffffa0000040000-0xfffffa0000041000` - HPET registers
* `0xfffffb0000000000-0xfffffb0100000000` - Kernel heap
* `0xfffffbffffffc000-0xfffffbffffffe000` - fault-handling stack
* `0xfffffc0000000000-???`                - physical memory map