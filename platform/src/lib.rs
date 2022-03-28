#![no_std]

use core::clone::Clone;
use core::cmp::Eq;
use core::fmt::Debug;
use core::hash::Hash;
use core::marker::{Send, Sized, Sync};

use embedded_graphics_core::prelude::{DrawTarget, OriginDimensions, RgbColor};

/// Wraps together all the types needed for a PlatypOS platform.
pub trait Platform: 'static + Sized + Eq + Clone + Hash + Debug + Send + Sync {
    // Display-related traits

    /// Display color type, to enforce that the display must be RGB-capable
    type DisplayColor: RgbColor;
    /// Display error type, to enforce that it must implement Debug
    type DisplayError: Debug;

    /// Graphics implementation for this platform
    type Display: DrawTarget<Color = Self::DisplayColor, Error = Self::DisplayError>
        + OriginDimensions;
}
