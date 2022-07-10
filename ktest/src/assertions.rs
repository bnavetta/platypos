use core::fmt::{self, Arguments};

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
        let lhs = $left;
        let rhs = $right;
        if lhs != rhs {
            $crate::assertions::report_eq_failure(file!(), line!(), column!(), stringify!($left), lhs, stringify!($right), rhs);
            return $crate::Outcome::Fail;
        }
    };
    ($left:expr, $right:expr, $(arg:tt)+) => {
        let lhs = $left;
        let rhs = $right;
        if lhs != rhs {
            $crate::assertions::report_failure(file!(), line!(), column!(), format_args!($(arg)+));
            return $crate::Outcome::Fail;
        }
    }
}

/// Called to report an assertion failure
#[doc(hidden)]
pub fn report_failure(file: &str, line: u32, column: u32, args: Arguments) {
    tracing::error!(
        "Assertion failed: '{}' at {}:{}:{}",
        args,
        file,
        line,
        column
    );
}

#[doc(hidden)]
pub fn report_eq_failure<T: fmt::Debug>(
    file: &str,
    line: u32,
    column: u32,
    left_expr: &str,
    left_value: T,
    right_expr: &str,
    right_value: T,
) {
    tracing::error!(
        "Assertion failed: '{left_expr}' did not equal '{right_expr}'\nleft: {left_value:?}\nright: {right_value:?}\nat {file}:{line}:{column}",
    );
}
