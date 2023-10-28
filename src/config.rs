use anyhow::Result;
use log::debug;
use serde::{Deserialize, Serialize};
use std::env;
use tokio::{
    fs::{metadata, OpenOptions},
    io::AsyncReadExt,
};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub upload_dir: String,
    pub temp_dir: String,
    pub prefix_length: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 4040,
            upload_dir: "files".to_string(),
            temp_dir: "temp".to_string(),
            prefix_length: 8,
        }
    }
}

pub async fn load() -> Result<Config> {
    let config_file = env::var("BINGUS_CONFIG").unwrap_or("config.toml".to_string());
    debug!("Loading configuration from {}", config_file);
    let metadata = metadata(&config_file).await?;
    let mut file = OpenOptions::new().read(true).open(&config_file).await?;
    let mut buf = String::with_capacity(metadata.len() as usize);
    file.read_to_string(&mut buf).await?;
    let config: Config = toml::from_str(buf.as_str())?; // this crate is torture
    Ok(config)
}
