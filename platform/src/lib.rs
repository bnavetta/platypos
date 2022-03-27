#![no_std]

use core::clone::Clone;
use core::cmp::Eq;
use core::fmt::Debug;
use core::hash::Hash;
use core::marker::{Send, Sized, Sync};

/// Wraps together all the types needed for a PlatypOS platform.
pub trait Platform: 'static + Sized + Eq + Clone + Hash + Debug + Send + Sync {}
