use std::fmt;

use clap::ArgEnum;

#[derive(Debug, Clone, Copy, ArgEnum)]
pub enum Platform {
    X86_64,
}

impl Platform {
    pub fn name(self) -> &'static str {
        match self {
            Platform::X86_64 => "x86_64",
        }
    }
}

impl fmt::Display for Platform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.name())
    }
}
