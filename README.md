# PlatypOS

PlatypOS is a collection of experimental code that may become an operating system one day.

Across several years and several iterations, some of the ideas it's tried out:

* Ergonomic zero-cost hardware abstraction layers using Rust's type system (mostly traits and generics) and conditional compilation
* Kernel developer tooling, including [tracing](https://github.com/tokio-rs/tracing) for structured logging and diagnostics, an in-kernel [test framework](./ktest),
  and tight GDB integration
* Various bootloading mechanisms - a custom UEFI binary to load ELF files, the [bootloader crate](https://github.com/rust-osdev/bootloader/),
  and [coreboot](https://www.coreboot.org/)and [u-boot](https://github.com/u-boot/u-boot) for RISC-V
* Different memory allocators including a [Buddy allocator](https://en.wikipedia.org/wiki/Buddy_memory_allocation) and a linked list of allocation ranges
  inspired by [Fuschia](https://cs.opensource.google/fuchsia/fuchsia/+/main:zircon/kernel/phys/lib/memalloc/include/lib/memalloc/pool.h).

Long-term, it's intended to be an OS for running servers:

* Processes are guaranteed a certain amount of dedicated CPU and memory. This is based on the assumption that, given enough traffic to fully utilize those resources,
  it's operationally easier to have predictability than lots of bust capacity. In particular, this should avoid variable tail latency due to CPU throttling and
  swapping.
* An almost entirely context-switch-free system call API, similar to `io_uring`, that should fit nicely into higher-level async runtimes like Rust futures.
* Hopefully, these two things build on each other. For example, applications could have complete control over their CPU cores (no preemption by the OS for scheduling),
  and pass work over to dedicated kernel cores asynchronously.
* I'd like to also support message-passing with capabilities/handles for security. This would be how the kernel grants resources to processes (particularly device access),
  but also accessible to userspace. Users could implement things like a log-shipping daemon that passes out handles tied to application log namespaces or an object
  storage API that produces opaque references which are still shareable.

## Current Status

PlatypOS is frequently broken. I'm currently in the process of rebuilding the tracing system to use a dedicated I/O worker task and lock-free shared state, so that it's safely
usable from interrupt handlers and the memory allocator.

When it does build, the [justfile](https://github.com/casey/just) has recipes for most common tasks:

* Run in QEMU: `just run`
* Run in-kernel unit tests: `just test`
* Run [Loom](https://github.com/tokio-rs/loom/) concurrency tests (on the host platform): `just loom`

For extra configuration (such as running under GDB), call the [xtask](https://github.com/matklad/cargo-xtask) tool directly (e.g. `cargo xtask run --debugger`).

This requires, Rust, Just, QEMU, and GDB.