#![feature(async_closure)]

mod config;

use std::{fmt::Debug, path::Path, sync::Arc};

use crate::config::Config;
use anyhow::Result;
use bingus_http::{
    cool_macro,
    handler::Handler,
    header::{HeaderName, Headers},
    App, Request, Response,
};
use colored::Colorize;
use log::{debug, info, trace, LevelFilter};
use path_dedot::ParseDot;
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

fn serve_static<T: Clone + Debug + Send + Sync + 'static>(root: &'static str) -> impl Handler<T> {
    async move |request: Request<T>| {
        let path = request.request.path.trim_matches('/');

        let parsed_path = match Path::new(path).parse_dot() {
            Ok(file) => file,
            Err(error) => {
                debug!("{:?} {:?}", request, error);
                return Ok(Response::from_code(400));
            }
        };
        let mut file_path = Path::new(root).join(parsed_path);

        let metadata = match tokio::fs::metadata(&file_path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                debug!("{:?} {:?}", request, error);
                match error.kind() {
                    tokio::io::ErrorKind::NotFound => {
                        return Ok(Response::from_code(404));
                    }
                    _ => return Err(error.into()),
                }
            }
        };

        if metadata.is_dir() {
            file_path = file_path.join("index.html");
        }

        let file = match OpenOptions::new().read(true).open(&file_path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                debug!("{:?} {:?}", request, error);
                match error.kind() {
                    tokio::io::ErrorKind::NotFound => {
                        return Ok(Response::from_code(404));
                    }
                    _ => return Err(error.into()),
                }
            }
        };

        let metadata = file.metadata().await?;

        if !metadata.is_file() {
            return Ok(Response::from_code(403));
        }

        let mime_type = mime_guess::from_path(file_path)
            .first()
            .unwrap_or(mime::APPLICATION_OCTET_STREAM)
            .to_string();

        Ok(Response {
            headers: Headers::from([
                (HeaderName::from("Content-Type"), mime_type),
                (
                    HeaderName::from("Content-Length"),
                    metadata.len().to_string(),
                ),
            ]),
            status_code: 200,
            body: Box::new(file),
        })
    }
}

fn serve_static_param<T: Clone + Debug + Send + Sync + 'static>(
    root: &'static str,
    param: &'static str,
) -> impl Handler<T> {
    async move |request: Request<T>| {
        let path = match request.params.get(param) {
            Some(p) => p,
            _ => {
                return Ok(Response::from_code(400));
            }
        };

        let parsed_path = match Path::new(path).parse_dot() {
            Ok(file) => file,
            Err(error) => {
                debug!("{:?} {:?}", request, error);
                return Ok(Response::from_code(400));
            }
        };
        let mut file_path = Path::new(root).join(parsed_path);

        let metadata = match tokio::fs::metadata(&file_path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                debug!("{:?} {:?}", request, error);
                match error.kind() {
                    tokio::io::ErrorKind::NotFound => {
                        return Ok(Response::from_code(404));
                    }
                    _ => return Err(error.into()),
                }
            }
        };

        if metadata.is_dir() {
            file_path = file_path.join("index.html");
        }

        let file = match OpenOptions::new().read(true).open(&file_path).await {
            Ok(metadata) => metadata,
            Err(error) => {
                debug!("{:?} {:?}", request, error);
                match error.kind() {
                    tokio::io::ErrorKind::NotFound => {
                        return Ok(Response::from_code(404));
                    }
                    _ => return Err(error.into()),
                }
            }
        };

        let metadata = file.metadata().await?;

        if !metadata.is_file() {
            return Ok(Response::from_code(403));
        }

        let mime_type = mime_guess::from_path(file_path)
            .first()
            .unwrap_or(mime::APPLICATION_OCTET_STREAM)
            .to_string();

        Ok(Response {
            headers: Headers::from([
                (HeaderName::from("Content-Type"), mime_type),
                (
                    HeaderName::from("Content-Length"),
                    metadata.len().to_string(),
                ),
            ]),
            status_code: 200,
            body: Box::new(file),
        })
    }
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
        return Ok(Response::from_code(503));
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
