#![feature(async_closure, fs_try_exists, io_error_more, let_chains)]

mod config;
mod silly;

use crate::config::Config;
use crate::silly::*;
use anyhow::Result;
use axum::{
    body::Body,
    extract::{ConnectInfo, DefaultBodyLimit, Path, Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{from_fn_with_state, Next},
    response::Response,
    routing::{get, get_service, put},
    Router,
};
use futures::TryStreamExt;
use humansize::{format_size, DECIMAL};
#[cfg(target_os = "linux")]
#[cfg(feature = "fallocate")]
use libc::{fallocate, strerror};
use owo_colors::{OwoColorize, Stream::Stderr};
use serde::Serialize;
#[cfg(target_os = "linux")]
#[cfg(feature = "fallocate")]
use std::{ffi::CStr, os::fd::AsRawFd};
use std::{
    fs::read_dir,
    path,
    sync::{Arc, RwLock},
    time::Duration,
};
use std::{io, net::SocketAddr};
use tokio::{
    fs::try_exists,
    net::TcpListener,
    time::{sleep, Instant},
};
use tokio_util::io::StreamReader;
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{compression::Compression, services::ServeDir, trace::TraceLayer};
#[allow(unused_imports)]
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Debug, Clone, Serialize)]
struct Stats {
    pub max_file_size: usize,
    pub files_stored: u64,
    pub storage_used: u64,
}

#[derive(Debug)]
struct AppState {
    pub config: Config,
    pub stats: RwLock<Stats>,
}

type ArcState = Arc<AppState>;

macro_rules! silly {
    ($code:ident) => {
        (StatusCode::$code, StatusCode::$code.to_string())
    };
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

async fn get_stats(State(state): State<ArcState>) -> String {
    serde_json::to_string(&*state.stats.read().unwrap()).unwrap()
}

async fn upload(
    State(state): State<ArcState>,
    ConnectInfo(connect_info): ConnectInfo<SocketAddr>,
    Path(path): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Result<String, (StatusCode, String)> {
    let file_size = match headers
        .get("content-length")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<usize>().ok())
    {
        Some(content_length) => content_length,
        None => return Err(silly!(BAD_REQUEST)),
    };

    if file_size > state.config.max_file_size {
        return Err(silly!(PAYLOAD_TOO_LARGE));
    }

    if path.len() > state.config.max_file_name_length {
        return Err(silly!(BAD_REQUEST));
    }

    let file_name = if state.config.prefix_length > 0 {
        format!(
            "{}.{}",
            get_random_prefix(state.config.prefix_length),
            sanitize_file_name(&path),
        )
    } else {
        path
    };

    let file_path = path::Path::new(&state.config.upload_dir).join(&file_name);

    if match tokio::fs::try_exists(&file_path).await {
        Ok(exists) => exists,
        Err(err) => return Err((StatusCode::INTERNAL_SERVER_ERROR, err.to_string())),
    } {
        return Err(silly!(CONFLICT));
    }

    if let Err(err) = async {
        info!(
            "{} is uploading file {} ({})",
            if state.config.behind_proxy {
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
            .append(false)
            .open(&file_path)
            .await?;

        #[cfg(target_os = "linux")]
        #[cfg(feature = "fallocate")]
        if file_size > 0 {
            trace!(
                "fallocating {} for '{}'",
                format_size(file_size, DECIMAL),
                file_name
            );
            let fd = out_file.as_raw_fd();
            unsafe {
                if fallocate(fd, 0, 0, file_size.try_into().unwrap()) == -1 {
                    let errno = *libc::__errno_location();
                    error!(
                        "Error while fallocating: {}",
                        CStr::from_ptr(strerror(errno)).to_string_lossy()
                    );
                    if errno == libc::ENOTSUP {
                        todo!()
                    } else {
                        return Err(io::Error::from_raw_os_error(errno));
                    }
                }
            };
        }

        let mut reader = StreamReader::new(
            body.into_data_stream()
                .map_err(|err| io::Error::new(io::ErrorKind::Other, err)),
        );

        tokio::io::copy(&mut reader, &mut out_file).await?;

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

        Err((
            match err.kind() {
                io::ErrorKind::FilesystemQuotaExceeded => StatusCode::INSUFFICIENT_STORAGE,
                _ => StatusCode::INTERNAL_SERVER_ERROR,
            },
            err.to_string(),
        ))
    } else {
        Ok(file_name)
    }
}

async fn logger(
    State(state): State<ArcState>,
    connect_info: ConnectInfo<SocketAddr>,
    request: Request,
    next: Next,
) -> Response {
    let ip = if state.config.behind_proxy {
        get_ip(request.headers())
    } else {
        None
    }
    .unwrap_or_else(|| connect_info.ip().to_string());

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
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "bingus_files=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    #[cfg(not(target_os = "linux"))]
    #[cfg(feature = "fallocate")]
    warn!("the 'fallocate' feature only works on linux");

    let config = match config::load().await {
        Ok(config) => {
            info!("Loaded configuration from {}", config.1.display());
            config.0
        }
        Err(error) => {
            error!("Error loading configuration: {}", error);
            info!("Using default configuration");
            Config::default()
        }
    };

    debug!("{:#?}", config);

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
                Router::new().route(
                    "/:file",
                    put(upload)
                        .layer(DefaultBodyLimit::max(config.max_file_size))
                        .with_state(state.clone()),
                ),
            ),
        )
        .route("/stats", get(get_stats))
        .layer(from_fn_with_state(state.clone(), logger))
        .layer(TraceLayer::new_for_http())
        .with_state(state.clone());
    let app = if config.concurrency_limit != 0 {
        app.layer(ConcurrencyLimitLayer::new(config.concurrency_limit))
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

    let address = (config.host.as_str(), config.port);
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
