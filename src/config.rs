use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use serde::Deserialize;
use thiserror::Error;
use tokio::{fs::OpenOptions, io::AsyncReadExt};
use tracing::debug;

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct HttpConfig {
    pub host: String,
    pub port: u16,
    pub concurrency_limit: usize,
    pub behind_proxy: bool,
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 4040,
            concurrency_limit: 512,
            behind_proxy: false,
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub http: HttpConfig,
    pub upload_dir: String,
    pub temp_dir: String,
    pub prefix_length: usize,
    pub max_file_size: usize,
    pub max_file_name_length: usize,
    pub stats_interval: u64,
    pub fallocate: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            upload_dir: "files".to_string(),
            temp_dir: "temp".to_string(),
            prefix_length: 8,
            max_file_size: 1_000_000_000,
            max_file_name_length: 200,
            stats_interval: 60,
            http: Default::default(),
            fallocate: true,
        }
    }
}

#[derive(Debug, Error)]
pub enum FindConfigError {
    #[error("No configuration file found")]
    NoneFoundError,
    #[error(transparent)]
    IoError(#[from] std::io::Error),
}

pub async fn load_from(config_file: &Path) -> Result<Config> {
    let mut file = OpenOptions::new().read(true).open(config_file).await?;
    let metadata = file.metadata().await?;
    let mut buf = String::with_capacity(metadata.len() as usize);
    file.read_to_string(&mut buf).await?;
    Ok(toml::from_str(buf.as_str())?)
}

pub async fn load() -> Result<(Config, PathBuf)> {
    let config_file = find_config()?;
    Ok((load_from(&config_file).await?, config_file))
}

pub fn find_config() -> Result<PathBuf, FindConfigError> {
    if let Ok(env_var) = env::var("BINGUS_CONFIG") {
        Ok(PathBuf::from(&env_var))
    } else {
        let config_dir = if cfg!(target_os = "windows") {
            PathBuf::from(&env::var("APPDATA").unwrap_or_default())
        } else {
            match env::var("XDG_CONFIG_HOME") {
                Ok(config_home) => PathBuf::from(&config_home),
                Err(_) => PathBuf::from(&env::var("HOME").unwrap_or_default()).join(".config"),
            }
        };

        let locations = [
            PathBuf::from("config.toml"),
            config_dir.join("bingus-files").join("config.toml"),
            #[cfg(not(target_os = "windows"))]
            PathBuf::from("/")
                .join("etc")
                .join("bingus-files")
                .join("config.toml"),
        ];
        for path in locations {
            debug!("checking if configuration exists at: {}", path.display());
            if fs::try_exists(&path)? {
                return Ok(path);
            }
        }

        Err(FindConfigError::NoneFoundError)
    }
}
