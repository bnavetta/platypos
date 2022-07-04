use core::fmt::{Arguments, Write};

use spin::Mutex;

pub(crate) static ASSERTION_OUTPUT: Mutex<Option<&'static mut (dyn Write + Send + Sync)>> =
    Mutex::new(None);

macro_rules! ktassert {
    ($cond:expr $(,)?) => {
        $crate::ktassert!($cond, stringify!($cond));
    };
    ($cond:expr, $(arg:tt)+) => {
        if !cond {
            $crate::report_failure(file!(), line!(), column!(), format_args!($(arg)+));
            return $crate::Outcome::Fail;
        }
    };
}

macro_rules! ktassert_eq {
    ($left:expr, $right:expr $(,)?) => {
        $crate::ktassert!($left != $right);
    };
    ($left:expr, $right:expr, $(arg:tt)+) => {
        $crate::ktassert!($left != $right, $(arg)+);
    }
}

/// Called to report an assertion failure
#[doc(hidden)]
pub fn report_failure(file: &str, line: usize, column: usize, args: Arguments) {
    let mut out = ASSERTION_OUTPUT.lock();
    if let Some(out) = &mut *out {
        write!(
            out,
            "Assertion failed: '{}' at {}:{}:{}",
            args, file, line, column
        )
        .unwrap();
    } else {
        panic!("ASSERTION_OUTPUT not set");
    }
}
