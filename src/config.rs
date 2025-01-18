use std::path::PathBuf;

use clap::Parser;
use serde::Deserialize;

#[derive(Debug, Clone, Parser)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    pub config: PathBuf,
}

impl Args {
    pub fn get_config(self) -> Config {
        Config::parse_file(self.config)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub token: String,
    pub storage: PathBuf,
}

impl Config {
    fn parse_file(path: PathBuf) -> Self {
        let yaml_content = std::fs::read_to_string(path).expect("Failed to read config file");
        serde_yml::from_str(&yaml_content).expect("Failed parsing config")
    }
}
