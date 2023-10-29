#![feature(async_closure)]

mod config;
mod serve_static;

use crate::serve_static::{serve_static, serve_static_param};

use std::{path::Path, sync::Arc};

use crate::config::Config;
use anyhow::Result;
use bingus_http::{cool_macro, header::HeaderName, App, Request, Response};
use colored::Colorize;
use log::{debug, info, trace, LevelFilter};
use rand::Rng;
use tokio::{
    fs::{try_exists, OpenOptions},
    io::{AsyncReadExt, AsyncWriteExt},
};

fn sanitize_file_name(name: &str) -> String {
    name.replace(
        ['/', '\\', '&', '?', '"', '\'', '*', '~', '|', ':', '<', '>'],
        "_",
    )
    .to_string()
}

async fn upload(mut request: Request<Arc<Config>>) -> Result<Response> {
    let file_name = request.params.get("file").expect("What");
    let random_prefix: String = rand::thread_rng()
        .sample_iter(rand::distributions::Alphanumeric)
        .take(request.state.prefix_length as usize)
        .map(char::from)
        .collect();
    let target_name = format!("{}.{}", random_prefix, sanitize_file_name(file_name));

    if try_exists(Path::new(&request.state.upload_dir).join(&target_name)).await? {
        // We got extremely unlucky (62^8)
        return Ok(Response::from_code(500));
    }

    if let Some(size) = request
        .request
        .headers
        .get(&HeaderName::from("Content-Length"))
        .and_then(|s| s.parse::<usize>().ok())
    {
        info!(
            "({:#?}) Uploading {} as {} ({})",
            request.address,
            file_name.bold(),
            target_name.bold(),
            humansize::format_size(size, humansize::DECIMAL)
        );

        if size > 0 {
            let mut buf = [0u8; 8192];
            let mut total = 0;
            let mut file = OpenOptions::new()
                .write(true)
                .create(true)
                .open(Path::new(&request.state.upload_dir).join(&target_name))
                .await?;
            loop {
                let read = request.request.body.read(&mut buf).await?;
                if read == 0 {
                    break;
                }
                let _written = file.write(&buf[..read]).await?;
                total += read;
                trace!("read/wrote {} bytes, {} total", read, total);
                if total >= size {
                    break;
                }
            }

            file.flush().await?;
        } else {
            tokio::fs::File::create(Path::new(&request.state.upload_dir).join(&target_name))
                .await?;
        }

        Ok(Response::from(target_name))
    } else {
        Ok(Response::from_code(400))
    }
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
    let address = (config.host.clone(), config.port);

    if !try_exists(&config.upload_dir).await.unwrap() {
        tokio::fs::create_dir(&config.upload_dir).await.unwrap();
    }
    if !try_exists(&config.temp_dir).await.unwrap() {
        tokio::fs::create_dir(&config.temp_dir).await.unwrap();
    }

    let app = App::new(Arc::new(config.clone()))
        .add_handler(cool_macro!(PUT / :file), upload)
        .add_handler(cool_macro!(GET / *), serve_static("static"))
        .add_handler(
            cool_macro!(GET / file / :file),
            serve_static_param(config.upload_dir.leak(), "file"),
        );

    app.listen(address).await.unwrap();
}
