use std::collections::HashMap;
use std::env;
use std::fs::{read_to_string, File};
use std::io::{BufWriter, Write};
use std::path::Path;

use phf_codegen;
use toml;

struct PlatyposConfig {
    max_processors: usize,
    log_levels: HashMap<String, &'static str>,
}

impl PlatyposConfig {
    fn new(raw: toml::Value) -> PlatyposConfig {
        let mut log_levels = HashMap::new();
        if let Some(raw_max_levels) = raw.get("log_levels") {
            let levels = raw_max_levels
                .as_table()
                .expect("log_levels must be a table");
            for (key, value) in levels.into_iter() {
                log_levels.insert(key.clone(), to_level_string(value));
            }
        }

        let max_processors = raw
            .get("max_processors")
            .map(|r| r.as_integer().expect("max_processors must be an integer"))
            .unwrap_or(1);

        PlatyposConfig {
            log_levels,
            max_processors: max_processors as usize,
        }
    }
}

fn to_level_string(value: &toml::Value) -> &'static str {
    let value = value.as_str().expect("Log levels must be strings");
    match value.to_uppercase().as_str() {
        "OFF" => "LevelFilter::Off",
        "ERROR" => "LevelFilter::Error",
        "WARN" => "LevelFilter::Warn",
        "INFO" => "LevelFilter::Info",
        "DEBUG" => "LevelFilter::Debug",
        "TRACE" => "LevelFilter::Trace",
        _ => panic!("Unknown log level: {}", value),
    }
}

fn main() {
    let config_file = env::current_dir().unwrap()
        .parent().unwrap()
        .join("platypos.toml");

    println!("cargo:rerun-if-changed={}", config_file.display());

    let contents = read_to_string(config_file).unwrap();
    let config = PlatyposConfig::new(contents.parse::<toml::Value>().expect("Invalid TOML"));

    let output = Path::new(&env::var("OUT_DIR").unwrap()).join("config.rs");

    let mut file = BufWriter::new(File::create(output).unwrap());

    write!(
        &mut file,
        "static LOG_LEVEL_FILTERS: phf::Map<&'static str, LevelFilter> = "
    )
        .unwrap();

    let mut builder = phf_codegen::Map::new();
    for (target, max_level) in config.log_levels.iter() {
        builder.entry(target.as_str(), max_level);
    }
    builder.build(&mut file).unwrap();
    writeln!(&mut file, ";").unwrap();

    writeln!(
        &mut file,
        "const MAX_PROCESSORS: usize = {};",
        config.max_processors
    )
        .unwrap();
}
