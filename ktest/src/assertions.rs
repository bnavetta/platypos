use core::fmt::Arguments;

#[macro_export]
macro_rules! ktassert {
    ($cond:expr) => {
        if !$cond {
            $crate::assertions::report_failure(file!(), line!(), column!(), format_args!("{}", stringify!($cond)));
            return $crate::Outcome::Fail;
        }
    };
    ($cond:expr, $(arg:tt)+) => {
        if !$cond {
            $crate::assertions::report_failure(file!(), line!(), column!(), format_args!($(arg)+));
            return $crate::Outcome::Fail;
        }
    };
}

#[macro_export]
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
pub fn report_failure(file: &str, line: u32, column: u32, args: Arguments) {
    defmt::println!(
        "Assertion failed: '{}' at {=str}:{=u32}:{=u32}",
        defmt::Display2Format(&args),
        file,
        line,
        column
    );
}
