#![no_std]

use log::LevelFilter;
use phf;

include!(concat!(env!("OUT_DIR"), "/config.rs"));

// Wrap in a struct for documentation and autocomplete. Also makes it easier to eventually support
// a kernel command line

/// PlatypOS configuration
pub struct Config;

impl Config {
    pub fn max_processors() -> usize {
        MAX_PROCESSORS
    }

    pub fn log_settings() -> &'static phf::Map<&'static str, LevelFilter> {
        &MAX_LOG_LEVELS
    }
}
