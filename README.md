# PlatypOS

PlatypOS a very-WIP microkernel.

## Organization

PlatypOS is split across several crates:

* `platypos_config` - build-time configuration such as the maximum number of supported processors
* `platypos_test` - minimal no_std testing framework
* `platypos_kernel` - the kernel itself

Within the kernel, platform-specific code is in the `platform` module, which is conditionally compiled to use the
appropriate implementation for the target system.

## Address Space

On x86-64, PlatypOS uses the conventional higher-half kernel layout. See
[Harvard's OS memory layout notes](https://read.seas.harvard.edu/cs161-18/doc/memory-layout/) and
[the OSDev wiki](https://wiki.osdev.org/Higher_Half_Kernel) for more information.

* The kernel is loaded into memory at 0xffffffff80000000
* All of physical memory is mapped into the kernel address space at 0xffff800000000000
* The loader information structure is at 0xffffffff70000000 (max 16 MiB)
* The kernel's initial stack is from 0xffffffff71000000 to 0xffffffff71001000 (1 page)

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