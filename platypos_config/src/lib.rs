#![no_std]
#![allow(clippy::all)]

use log::LevelFilter;
use phf;

include!(concat!(env!("OUT_DIR"), "/config.rs"));

// The generated code is hidden by these functions for better autocomplete and flexibility (e.g. for
// a future kernel command line)

pub fn max_processors() -> usize {
    MAX_PROCESSORS
}

pub fn log_levels() -> &'static phf::Map<&'static str, LevelFilter> {
    &LOG_LEVEL_FILTERS
}

pub fn build_revision() -> &'static str {
    GIT_REVISION
}
