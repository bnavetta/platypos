//! System topology and capabilities.
//!
//! Most system topology information is only known at runtime and needs to be detected. This includes
//! the number and layout of CPU cores, what peripheral devices are available, and some aspects of
//! processor hardware.
//!
//! Not all feature detection is done here, particularly not for CPU features. For the most part,
//! those are detected through CPUID or model-specific registers and handled by the feature-specific
//! drivers.

pub mod acpi;
