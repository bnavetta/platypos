use std::collections::HashMap;
use std::env;
use std::fs::{read_to_string, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use std::process::Command;

use phf_codegen;
use serde::Deserialize;
use toml;

#[derive(Deserialize)]
struct PlatyposConfig {
    max_processors: usize,
    log_levels: HashMap<String, String>,
}

fn to_level_string(value: &str) -> &'static str {
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

fn git_revision() -> String {
    let output = Command::new("git")
        .arg("describe")
        .arg("--all")
        .arg("--always")
        .arg("--dirty")
        .arg("--long")
        .output().expect("Could not run git describe");

    assert!(output.status.success(), "git describe failed");

    String::from_utf8(output.stdout)
        .expect("Invalid output from git describe")
        .trim()
        .to_string()
}

fn main() {
    let config_file = env::current_dir().unwrap()
        .parent().unwrap()
        .join("platypos.toml");

    println!("cargo:rerun-if-changed={}", config_file.display());

    let contents = read_to_string(config_file).unwrap();
    let config: PlatyposConfig = toml::from_str(&contents).expect("Invalid configuration");

    let output = Path::new(&env::var("OUT_DIR").unwrap()).join("config.rs");

    let mut file = BufWriter::new(File::create(output).unwrap());

    write!(
        &mut file,
        "static LOG_LEVEL_FILTERS: phf::Map<&'static str, LevelFilter> = "
    )
        .unwrap();

    let mut builder = phf_codegen::Map::new();
    for (target, max_level) in config.log_levels.iter() {
        builder.entry(target.as_str(), to_level_string(max_level));
    }
    builder.build(&mut file).unwrap();
    writeln!(&mut file, ";").unwrap();

    writeln!(
        &mut file,
        "const MAX_PROCESSORS: usize = {};",
        config.max_processors
    )
        .unwrap();

    writeln!(&mut file, "const GIT_REVISION: &'static str = \"{}\";", git_revision()).unwrap();
}
