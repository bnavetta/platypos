# PlatypOS

PlatypOS a very-WIP microkernel.

## Organization

PlatypOS uses a platform abstraction layer to keep as much as possible architecture-independent. This is implemented
across a number of sub-crates, inspired by [gfx-rs](https://github.com/gfx-rs/gfx).

* `platypos_pal` - types making up the platform abstraction API
* `platypos_platform_{x86_64, ...}` - implementation of the PAL for a particular platform
* `platypos_kernel` - platform-independent kernel code
* `platypos_{x86_64, ...}` - platform-specific entry point

The entry point crates are separated from `platypos_kernel` and their corresponding PAL implementations to prevent
circular dependencies. This allows `platypos_kernel` to have a target-specific hard dependency on the appropriate
PAL implementation instead of being explicitly generic over `platypos_pal::Platform` (which is hard to read and annoying
to pipe through). The entry points can then depend on the core kernel and work with the platform-specific bootloader,
instead of pushing that into `kernel_core` or trying to set up circular dependencies. It also allows multiple entry
points per platform, such as BIOS and UEFI versions on x86-64.

This is a lot of crates, but I didn't have a better alternative than the PAL implementation - kernel core - entry point
sandwich.

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