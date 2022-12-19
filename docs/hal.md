# PlatypOS Hardware Abstraction Layer

The hardware abstraction layer (HAL) is a layer of indirection between the core PlatypOS kernel and platform-specific code. It's implemented using a mix of traits and conditional compilation.

The HAL interface is defined in the [`platypos_hal`](../hal) crate, consisting of platform API traits. For example, the `Topology` trait defines
how to access the system's topology, particularly to identify the current processor.

Each platform implements the HAL in a `platypos_hal-$platform` crate, such as [`platypos_hal-x86_64`](../hal-x86_64). These crates mirror the `hal` API, but with
platform-specific implementations.

Finally, the [`arch`](../kernel/src/arch) modules in the kernel instantiate the HAL. This is where the bootloader-specific entry points live, along with device setup. They also
reexport the HAL implementation crates for the rest of the kernel to use (this is why each crate must mirror the `hal` API).

Libraries like [`platypos_slab`](../slab) use HAL traits to stay completely platform-agnostic. Because Rust monomorphizes generics at compile-time, they can still benefit from 
platform-specific optimizations. For example, a call to `Topology::current_processor()` _should_ inline to a read of the x2APIC MSR on x86-64, and the `&self` parameter should
optimize away.

A problem with trait bounds is that they make storing library types (or really, any generic type) in global variables difficult. This matters because pretty much anything accessible
from an interrupt handler, like the [tracing subscriber](https://docs.rs/tracing/latest/tracing/subscriber/trait.Subscriber.html), must be in globals. This is where the `arch`
module's HAL reexports come in. The kernel can initialize any global state using types in `arch`, which will resolve to the right HAL implementation types at compile time.

## Open Problems

One wart with the HAL traits is that they leak implementation details. Types effectively declare which HAL APIs they use, because they have a type parameter for each.
Instead, it might be better to use a top-level `Platform` or `Api` trait, like [`gfx-hal`](https://github.com/gfx-rs/gfx) and its successor,
[`wgpu-hal`](https://github.com/gfx-rs/wgpu/tree/master/wgpu-hal). Any HAL-dependent shared code would have a single type parameter for the platform. I've tried this a little
in the past, and there were some ergonomic issues, but improvements to Rust generics might have eliminated them.

It might also be useful to have a container type for platform services that are available from early boot, like the interrupt controller and topology APIs. This would have
a `'static` lifetime, probably compile to a zero-sized type, and could easily be passed around. It might also be workable to have these globally-accessible APIs defined
in terms of methods that don't take `self`. If a given platform needs state, the `arch` module could ensure it's initialized during early boot. This ought to work pretty
well for APIs that compile to a special instruction or memory access (once "thread"-local storage is implemented), but shouldn't be over-used.

The kernel's address space is also somewhat undefined. Linux assumes all physical memory is mapped into the kernel, but there are security and portability reasons not to
do that. The `MemoryAccess` trait has some support for the idea of temporary and permanent memory mappings, but it's not the most user-friendly.