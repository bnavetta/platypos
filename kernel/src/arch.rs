#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "x86_64")]
pub use self::x86_64::*;

// See https://bitshifter.github.io/2020/05/07/conditional-compilation-in-rust/ for some ideas on how to organize this
// If keeping different platforms in sync becomes difficult, consider the "By
// modules addendum" approach
