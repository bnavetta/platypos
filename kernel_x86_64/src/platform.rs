//! x86-64 implementation of the [`Platform`] trait.

use platypos_platform::Platform;

#[derive(PartialEq, Eq, Clone, Copy, Hash, Debug)]
pub struct PlatformX86_64 {}

impl Platform for PlatformX86_64 {}
