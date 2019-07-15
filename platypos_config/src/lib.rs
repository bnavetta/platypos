#![no_std]

use log::LevelFilter;
use phf;

include!(concat!(env!("OUT_DIR"), "/config.rs"));

pub fn max_processors() -> usize {
    MAX_PROCESSORS
}

pub fn log_levels() -> &'static phf::Map<&'static str, LevelFilter> {
    &LOG_LEVEL_FILTERS
}