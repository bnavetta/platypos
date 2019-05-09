use serial_logger;

use bootloader::{entry_point, BootInfo};
use log::info;

use crate::qemu;

fn test_kernel_main(boot_info: &'static BootInfo) -> ! {
    super::init_core(boot_info);

    super::test_main();
    loop {}
}

entry_point!(test_kernel_main);

pub fn test_runner(tests: &[&dyn TestCase]) {
    info!("Running {} tests...", tests.len());

    for test in tests {
        info!("test {}...", test.name());
        test.run();
    }

    // Any tests that don't pass should panic, so if we get here they all passed
    info!("All tests passed!");

    qemu::exit(qemu::ExitCode::Success);
}

pub trait TestCase {
    fn name(&self) -> &'static str;

    fn run(&self);
}


#[macro_export]
macro_rules! tests {
    { $(
            test $name:ident {
                $($code:tt)*
            }
        )* } => {
        $(
            #[cfg(test)]
            #[allow(non_camel_case_types)]
            pub struct $name;

            #[cfg(test)]
            impl $crate::test::TestCase for $name {
                fn name(&self) -> &'static str {
                    concat!(module_path!(), "::", stringify!($name))
                }

                fn run(&self) {
                    $($code)*
                }
            }

            #[cfg(test)]
            #[test_case]
            #[allow(non_upper_case_globals)]
            pub static $name: $name = $name;
        )*
    }
}