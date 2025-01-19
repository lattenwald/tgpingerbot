use std::{net::SocketAddr, path::PathBuf};

use clap::Parser;
use serde::Deserialize;
use url::Url;

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
    pub bot: BotConfig,
    pub storage: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BotConfig {
    pub token: String,
    pub admin_id: Option<i64>,
    pub webhook: Option<WebhookConfig>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WebhookConfig {
    pub url: Url,
    pub socket: SocketAddr,
}

impl Config {
    fn parse_file(path: PathBuf) -> Self {
        let yaml_content = std::fs::read_to_string(path).expect("Failed to read config file");
        serde_yml::from_str(&yaml_content).expect("Failed parsing config")
    }
}
