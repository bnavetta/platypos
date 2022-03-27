use std::fmt;

use camino::Utf8Path;
use clap::ArgEnum;

#[derive(Debug, Clone, Copy, ArgEnum)]
pub enum Platform {
    X86_64,
}

static COMMON_BUILD_FLAGS: &[&str] = &[
    "-Zbuild-std=core,compiler_builtins,alloc",
    "-Zbuild-std-features=compiler-builtins-mem",
];

impl Platform {
    pub fn kernel_crate(self) -> &'static str {
        match self {
            Platform::X86_64 => "platypos_kernel_x86_64",
        }
    }

    pub fn target(self) -> &'static Utf8Path {
        Utf8Path::new("kernel_x86_64/x86_64-kernel.json")
    }

    pub fn kernel_manifest(self) -> &'static Utf8Path {
        Utf8Path::new("kernel_x86_64/Cargo.toml")
    }

    pub fn build_flags(self) -> &'static [&'static str] {
        COMMON_BUILD_FLAGS
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Platform::X86_64 => f.write_str("x86-64"),
        }
    }
}
