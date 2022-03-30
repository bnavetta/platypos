//! Platform abstraction layer for PlatypOS
#![no_std]

use core::clone::Clone;
use core::cmp::Eq;
use core::fmt::{self, Debug};
use core::hash::Hash;
use core::marker::{Send, Sized, Sync};

use embedded_graphics_core::prelude::{DrawTarget, OriginDimensions, RgbColor};

/// Wraps together all the types needed for a PlatypOS platform.
pub trait Platform: 'static + Sized + Eq + Clone + Hash + Debug + Send + Sync {
    // Processor control

    // TODO: rename to just handle interrupt stuff
    type Processor: Processor;

    // Display-related traits

    /// Display color type, to enforce that the display must be RGB-capable
    type DisplayColor: RgbColor;
    /// Display error type, to enforce that it must implement Debug
    type DisplayError: Debug;

    /// Graphics implementation for this platform
    type Display: DrawTarget<Color = Self::DisplayColor, Error = Self::DisplayError>
        + OriginDimensions;

    // Logging, tracing, and general observability

    /// Serial port writer, for sending log messages
    type Serial: fmt::Write + Send;
}

pub trait Processor {
    type InterruptGuard;

    /// Disable interrupts on the current processor, returning a guard value.
    /// When the guard is dropped, interrupts are reenabled.
    fn disable_interrupts() -> Self::InterruptGuard;

    /// Halts the processor until there's an interrupt
    fn halt_until_interrupted();
}
