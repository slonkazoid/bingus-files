use std::{
    env, fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use log::trace;
use serde::Deserialize;
use thiserror::Error;
use tokio::{fs::OpenOptions, io::AsyncReadExt};

#[derive(Deserialize, Debug, Clone)]
#[serde(default)]
pub struct Config {
    pub host: String,
    pub port: u16,
    pub upload_dir: String,
    pub temp_dir: String,
    pub prefix_length: usize,
    pub max_file_size: usize,
    pub max_file_name_length: usize,
    pub stats_interval: u64,
    pub behind_proxy: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 4040,
            upload_dir: "files".to_string(),
            temp_dir: "temp".to_string(),
            prefix_length: 8,
            max_file_size: 1_000_000_000,
            max_file_name_length: 100,
            stats_interval: 60,
            behind_proxy: false,
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

/// Find configuration file.
///
/// Checks in these locations:
///
/// 1. `$BINGUS_CONFIG`
/// 2. `config.toml`
/// 3. `$XDG_CONFIG_HOME/bingus-files/config.toml`
/// 4. `/etc/bingus-files/config.toml` (`nul` on windows)
///
/// # Errors
///
/// This function will return an error if there was a problem with IO, or if none of the files
/// exist.
pub fn find_config() -> Result<PathBuf, FindConfigError> {
    if let Ok(env_var) = env::var("BINGUS_CONFIG") {
        Ok(Path::new(&env_var).into())
    } else {
        let config_home = match env::var("XDG_CONFIG_HOME") {
            Ok(config_home) => Path::new(&config_home).to_path_buf(),
            Err(_) => Path::new(&env::var("HOME").unwrap_or_default()).join(".config"),
        };

        let locations = [
            Path::new("config.toml").to_path_buf(),
            config_home.join("bingus-files").join("config.toml"),
            #[cfg(not(target_os = "windows"))]
            Path::new("/")
                .join("etc")
                .join("bingus-files")
                .join("config.toml"),
        ];
        for path in locations {
            if fs::try_exists(&path)? {
                return Ok(path);
            } else {
                trace!("doesn't exist: {}", path.display());
            }
        }

        Err(FindConfigError::NoneFoundError)
    }
}
