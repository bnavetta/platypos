use std::fmt;

use camino::Utf8PathBuf;
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
    pub fn name(self) -> &'static str {
        match self {
            Platform::X86_64 => "x86_64",
        }
    }

    pub fn target(self) -> Utf8PathBuf {
        let name = self.name();
        Utf8PathBuf::from(format!("kernel/src/arch/{name}/{name}-kernel.json"))
    }

    pub fn build_flags(self) -> &'static [&'static str] {
        COMMON_BUILD_FLAGS
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Platform::X86_64 => f.write_str(self.name()),
        }
    }
}
