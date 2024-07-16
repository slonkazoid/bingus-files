#![feature(async_closure, io_error_more, let_chains, addr_parse_ascii)]

mod config;
mod silly;

use crate::config::{Config, FileEnum, FindConfigError};
use crate::silly::*;
use anyhow::Result;
use axum::{
    body::Body,
    extract::{ConnectInfo, Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::{IntoResponse, Response},
    routing::{get, get_service, put},
    Router,
};
use futures::TryStreamExt;
use humansize::{format_size, DECIMAL};
use owo_colors::{OwoColorize, Stream::Stderr};
use serde::Serialize;
use std::net::IpAddr;
use std::{
    fs::read_dir,
    path,
    str::FromStr,
    sync::{Arc, RwLock},
    time::Duration,
};
use std::{io, net::SocketAddr};
use thiserror::Error;
use tokio::{
    fs::try_exists,
    net::TcpListener,
    time::{sleep, Instant},
};
use tokio_util::io::StreamReader;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{compression::Compression, services::ServeDir};
use tracing::level_filters::LevelFilter;
use tracing::{debug, error, info, trace};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

macro_rules! silly {
    ($code:ident) => {
        (StatusCode::$code, StatusCode::$code.to_string())
    };
}

const DEFAULT_LOG_PATH: &str = "bingus-files_%Y-%m-%dT%H:%M:%S%:z.log";

#[derive(Debug, Clone, Serialize)]
struct Stats {
    pub max_file_size: u64,
    pub files_stored: u64,
    pub storage_used: u64,
}

#[derive(Debug)]
struct AppState {
    pub config: Config,
    pub stats: RwLock<Stats>,
}

type ArcState = Arc<AppState>;

#[derive(Debug, Error)]
enum AppError {
    #[error("Bad request")]
    BadRequest,
    #[error("File name too long")]
    NameTooLong,
    #[error("File was above max size")]
    FileAboveMaxSize,
    #[error("File already exists")]
    Conflict,
    #[error(transparent)]
    IoError(#[from] io::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        error!("{}", self);
        match self {
            Self::BadRequest => silly!(BAD_REQUEST),
            Self::NameTooLong => (StatusCode::BAD_REQUEST, "File name too long".to_string()),
            Self::FileAboveMaxSize => silly!(PAYLOAD_TOO_LARGE),
            Self::Conflict => silly!(CONFLICT),
            Self::IoError(err) => match err.kind() {
                io::ErrorKind::FilesystemQuotaExceeded => silly!(INSUFFICIENT_STORAGE),
                _ => silly!(INTERNAL_SERVER_ERROR),
            },
        }
        .into_response()
    }
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

async fn get_stats(State(state): State<ArcState>) -> Slonkable<Stats> {
    state.stats.read().unwrap().clone().into()
}

async fn upload(
    State(state): State<ArcState>,
    ConnectInfo(connect_info): ConnectInfo<SocketAddr>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<String, AppError> {
    let file_size = match headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
    {
        Some(content_length) => content_length,
        None => return Err(AppError::BadRequest),
    };

    if file_size > state.config.max_file_size {
        return Err(AppError::FileAboveMaxSize);
    }

    if path.len() > state.config.max_file_name_length {
        return Err(AppError::NameTooLong);
    }

    let file_name = if state.config.prefix_length > 0 {
        format!(
            "{}.{}",
            get_random_prefix(state.config.prefix_length),
            sanitize_file_name(&path),
        )
    } else {
        let new_name = sanitize_file_name(&path);
        if new_name == "." || new_name == ".." {
            return Err(AppError::BadRequest);
        }
        // TODO: con, prn, aux, etc. on windows
        new_name
    };

    let file_path = path::Path::new(&state.config.upload_dir).join(&file_name);

    if tokio::fs::try_exists(&file_path).await? {
        return Err(AppError::Conflict);
    }

    if let Err(err) = async {
        info!(
            "{} is uploading file {} ({})",
            if state.config.http.behind_proxy {
                get_ip(&headers)
            } else {
                None
            }
            .unwrap_or_else(|| connect_info.ip().to_string()),
            file_name.if_supports_color(Stderr, |text| text.bold()),
            format_size(file_size, DECIMAL),
        );

        let mut out_file = tokio::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&file_path)
            .await?;

        if file_size > 0 && state.config.allocate {
            debug!(
                "allocating {} for '{}'",
                format_size(file_size, DECIMAL),
                file_name
            );

            out_file.set_len(file_size).await?;
        }

        let mut reader = StreamReader::new(
            body.into_data_stream()
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err)),
        );

        tokio::io::copy(&mut reader, &mut out_file).await?;

        let mut stats = state.stats.write().unwrap();

        stats.files_stored += 1;
        stats.storage_used += file_size;

        Ok::<_, io::Error>(())
    }
    .await
    {
        trace!("cleaning up failed upload of '{}'", file_name);
        match tokio::fs::try_exists(&file_path).await {
            Ok(exists) => {
                if exists {
                    if let Err(err) = tokio::fs::remove_file(&file_path).await {
                        error!(
                            "Error while removing file '{}': {}",
                            file_path.display(),
                            err
                        );
                    };
                }
            }
            Err(err) => {
                error!(
                    "Error while checking if file '{}' exists: {}",
                    file_path.display(),
                    err
                )
            }
        };

        Err(err.into())
    } else {
        Ok(urlencoding::encode(&file_name).to_string())
    }
}

async fn logger(
    State(state): State<ArcState>,
    connect_info: ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let ip = if state.config.http.behind_proxy {
        get_ip(request.headers())
            .and_then(|x| IpAddr::parse_ascii(x.as_bytes()).ok())
            .unwrap_or_else(|| connect_info.ip())
    } else {
        connect_info.ip()
    };

    let path = request.uri().path().to_owned();
    let method = request.method().to_owned();

    let start = Instant::now();
    let response = next.run(request).await;
    let elapsed = start.elapsed();

    let status_code = response.status().as_u16();

    info!(
        "({}) {} {} {} ({:#?})",
        ip,
        status_code.if_supports_color(Stderr, |text| text
            .style(color_status_code(status_code).bold())),
        method.if_supports_color(Stderr, |text| text.bold()),
        path,
        elapsed
    );

    response
}

#[tokio::main]
async fn main() {
    let config = match config::load().await {
        Ok(config) => {
            eprintln!("Loaded configuration from {}", config.1.display());
            config.0
        }
        Err(error) => {
            eprintln!("Error loading configuration: {}", error);
            if error.is::<FindConfigError>()
                && matches!(
                    error.downcast::<FindConfigError>().unwrap(),
                    FindConfigError::NoneFoundError
                )
            {
                eprintln!("Using default configuration");
                Config::default()
            } else {
                unimplemented!()
            }
        }
    };

    tracing_subscriber::registry()
        .with(LevelFilter::from_str(&config.logging.level).unwrap())
        .with(
            config
                .logging
                .stderr
                .then_some(tracing_subscriber::fmt::layer()),
        )
        .with(
            match &config.logging.file {
                FileEnum::Boolean(value) => value.then_some(DEFAULT_LOG_PATH),
                FileEnum::Path(value) => Some(value.as_str()),
            }
            .map(|path| {
                let time = chrono::Utc::now();
                let path = time.format(path).to_string();
                let file = std::fs::OpenOptions::new()
                    .append(true)
                    .create(true)
                    .open(path)
                    .unwrap();
                tracing_subscriber::fmt::layer()
                    .json()
                    .with_writer(file)
                    .with_ansi(false)
            }),
        )
        .init();

    debug!("{:#?}", &config);

    if !try_exists(&config.upload_dir).await.unwrap() {
        debug!("Creating upload directory");
        tokio::fs::create_dir_all(&config.upload_dir).await.unwrap();
    }
    if !try_exists(&config.temp_dir).await.unwrap() {
        debug!("Creating temp directory");
        tokio::fs::create_dir_all(&config.temp_dir).await.unwrap();
    }

    let stats = refresh_stats(&config).unwrap();

    let state = Arc::new(AppState {
        config: config.clone(),
        stats: RwLock::new(stats),
    });

    let serve_files = ServeDir::new(&config.upload_dir).precompressed_gzip();
    let serve_static =
        Compression::new(ServeDir::new(path::Path::new("static")).fallback(serve_files));

    let app = Router::new()
        .nest_service(
            "/",
            get_service(serve_static).fallback_service(
                Router::new().route("/:file", put(upload).with_state(state.clone())),
            ),
        )
        .route("/stats", get(get_stats))
        .layer(from_fn_with_state(state.clone(), logger))
        .with_state(state.clone());
    let app = if config.http.concurrency_limit != 0 {
        app.layer(ConcurrencyLimitLayer::new(config.http.concurrency_limit))
    } else {
        app
    };

    let state = state.clone();
    tokio::spawn(async move {
        loop {
            sleep(Duration::from_secs(state.config.stats_interval)).await;
            debug!("Refreshing stats");
            let stats = refresh_stats(&state.config).unwrap();
            *state.stats.write().unwrap() = stats;
        }
    });

    let address = (config.http.host.as_str(), config.http.port);
    let listener = TcpListener::bind(address).await.unwrap();
    let local_addr = listener.local_addr().unwrap();
    info!(
        "listening on http://{}:{}",
        local_addr.ip().bold(),
        local_addr.port().bold()
    );

    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .await
    .unwrap();
}
