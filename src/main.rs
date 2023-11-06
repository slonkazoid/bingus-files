#![feature(async_closure)]

mod config;
mod serve_static;

use crate::serve_static::{serve_static, serve_static_param};

use std::{fs::read_dir, path::Path, sync::Arc, time::Duration};

use crate::config::Config;
use anyhow::Result;
use bingus_http::{cool_macro, header::HeaderName, App, Request, Response};
use colored::Colorize;
use log::{debug, info, trace, LevelFilter};
use rand::Rng;
use serde::Serialize;
use tokio::{
    fs::{try_exists, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt, BufWriter},
    sync::RwLock,
    task,
    time::sleep,
};

#[derive(Debug, Clone, Serialize)]
struct Stats {
    pub max_file_size: u64,
    pub files_stored: u64,
    pub storage_used: u64,
}

#[derive(Debug)]
struct State {
    pub config: Config,
    pub stats: RwLock<Stats>,
}

fn refresh_stats(config: &Config) -> Result<Stats> {
    let files_dir = read_dir(&config.upload_dir)?;

    let mut files_stored = 0;
    let mut storage_used = 0;

    for file in files_dir {
        let file = file?;
        let metadata = file.metadata()?;

        if metadata.is_file() {
            files_stored += 1;
            storage_used += metadata.len();
        }
    }

    Ok(Stats {
        max_file_size: config.max_file_size,
        files_stored,
        storage_used,
    })
}

fn sanitize_file_name(name: &str) -> String {
    name.replace(
        ['/', '\\', '&', '?', '"', '\'', '*', '~', '|', ':', '<', '>'],
        "_",
    )
    .to_string()
}

fn get_random_prefix(length: usize) -> String {
    rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(length)
        .map(char::from)
        .collect()
}

async fn upload(mut request: Request<Arc<State>>) -> Result<Response> {
    let file_name = request.params.get("file").unwrap();
    let random_prefix = get_random_prefix(request.state.config.prefix_length as usize);
    let target_name = format!("{}.{}", random_prefix, sanitize_file_name(file_name));

    if try_exists(Path::new(&request.state.config.upload_dir).join(&target_name)).await? {
        // We got extremely unlucky (62^8 by default)
        return Ok(Response::from_code(500));
    }

    if let Some(size) = request
        .request
        .headers
        .get(&HeaderName::from("Content-Length"))
        .and_then(|s| s.parse::<u64>().ok())
    {
        let ip = if request.state.config.behind_proxy
            && request
                .request
                .headers
                .contains_key(&HeaderName::from("X-Forwarded-For"))
        {
            request
                .request
                .headers
                .get(&HeaderName::from("X-Forwarded-For"))
                .unwrap_or_else(|| unreachable!())
                .split(',')
                .next()
                .unwrap_or_else(|| unreachable!())
                .to_string()
        } else {
            request.address.ip().to_string()
        };
        info!(
            "({}) Uploading {} as {} ({})",
            ip,
            file_name.bold(),
            target_name.bold(),
            humansize::format_size(size, humansize::DECIMAL)
        );

        if size > request.state.config.max_file_size {
            return Ok(Response::from_code(400));
        }
        if size > 0 {
            let mut buf = [0u8; 8192];
            let mut total = 0;
            let file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(Path::new(&request.state.config.upload_dir).join(&target_name))
                .await?;
            let mut writer = BufWriter::new(file);
            loop {
                let read = request.request.body.read(&mut buf).await?;
                if read == 0 {
                    break;
                }
                let _written = writer.write(&buf[..read]).await?;
                total += read;
                trace!("read/wrote {} bytes, {} total", read, total);
                if total as u64 >= size {
                    break;
                }
            }

            writer.flush().await?;

            request.state.stats.write().await.storage_used += size;
        } else {
            tokio::fs::File::create(Path::new(&request.state.config.upload_dir).join(&target_name))
                .await?;
        }

        request.state.stats.write().await.files_stored += 1;

        Ok(Response::from(target_name))
    } else {
        Ok(Response::from_code(400))
    }
}

async fn get_stats(request: Request<Arc<State>>) -> Result<String> {
    Ok(serde_json::to_string(&*request.state.stats.read().await)?)
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let config = match config::load().await {
        Ok(config) => {
            info!("Loaded configuration");
            config
        }
        Err(error) => {
            debug!("Error loading configuration: {}", error);
            info!("Using default configuration");
            Config::default()
        }
    };
    debug!("{:#?}", config);

    let address = (config.host.clone(), config.port);

    if !try_exists(&config.upload_dir).await.unwrap() {
        debug!("Creating upload directory");
        tokio::fs::create_dir_all(&config.upload_dir).await.unwrap();
    }
    if !try_exists(&config.temp_dir).await.unwrap() {
        debug!("Creating temp directory");
        tokio::fs::create_dir_all(&config.temp_dir).await.unwrap();
    }

    let files_dir = config.upload_dir.clone().leak();
    let stats = refresh_stats(&config).unwrap();

    let state = Arc::new(State {
        config,
        stats: RwLock::new(stats),
    });

    let app = App::new(state.clone())
        .add_handler(cool_macro!(PUT / :file), upload)
        .add_handler(cool_macro!(GET / stats), get_stats)
        .add_handler(cool_macro!(GET / *), serve_static("static"))
        .add_handler(
            cool_macro!(GET / file / :file),
            serve_static_param(files_dir, "file"),
        );

    let state = state.clone();
    task::spawn(async move {
        loop {
            sleep(Duration::from_secs(state.config.stats_interval)).await;
            debug!("Refreshing stats");
            let stats = refresh_stats(&state.config).unwrap();
            *state.stats.write().await = stats;
        }
    });

    app.listen(address).await.unwrap();
}
