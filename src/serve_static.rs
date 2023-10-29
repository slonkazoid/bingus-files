use bingus_http::{
    handler::Handler,
    header::{HeaderName, Headers},
    Request, Response,
};
use log::debug;
use path_dedot::ParseDot;
use std::{fmt::Debug, path::Path};
use tokio::fs::OpenOptions;

pub fn serve_static<T: Clone + Debug + Send + Sync + 'static>(
    root: &'static str,
) -> impl Handler<T> {
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

pub fn serve_static_param<T: Clone + Debug + Send + Sync + 'static>(
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
