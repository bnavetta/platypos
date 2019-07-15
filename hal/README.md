# PlatypOS HAL

This is a Hardware Abstraction Layer for PlatypOS. It hides architecture- and
platform-specific implementation details from the rest of the kernel

## Design

### Why explicitly pass the `Platform` around?

The main reason is monomorphization. If there were some global variable for the
current platform, it'd have to be a trait object, probably something like
`static mut PLATFORM: &'static Platform`. That would mean all the platform method
calls would go through dynamic dispatch, even though the OS is only compiled for
one platform at a time. It would also require that `Platform` be object-safe, which
would (I think) mean returning trait objects for all the platform services instead of
associated types.

Not having a global `Platform` also gives the platform implementation more flexibility. At each
point where it passes a `Platform` to the rest of the kernel, it can choose how to do so. For example,
there might be a per-processor `Platform` instance, or a single global variable, or a new instance for
every use.