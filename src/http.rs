use crate::header::Headers;
use crate::{
    header::HeaderName,
    status::{color_status_code, StatusText},
};
use colored::Colorize;
use log::{debug, error, info, trace};
use std::fmt::format;
use std::{
    fmt::Debug,
    hint::unreachable_unchecked,
    io::{Cursor, ErrorKind},
    net::SocketAddr,
    sync::Arc,
    time::UNIX_EPOCH,
};
use thiserror::Error;
use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
        BufReader, BufWriter,
    },
    net::{TcpListener, TcpStream, ToSocketAddrs},
    sync::RwLock,
    task::{self, JoinError},
    time::Instant,
};

const MAX_HEADER_SIZE: u64 = 8192;
const MAX_METHOD_LEN: usize = 16;
const MAX_PATH_LEN: usize = 2048;

#[derive(Debug)]
pub struct Request<R: AsyncRead + Unpin + Debug> {
    pub headers: Headers,
    pub method: String,
    pub path: String,
    pub params: Option<String>,
    pub body: R,
}

#[derive(Debug)]
pub struct Response<R: AsyncRead + Unpin + Debug> {
    pub headers: Headers,
    pub status_code: u32,
    pub body: R,
}

pub trait AsyncReadDebugSendSync: AsyncRead + Debug + Send + Sync {}
impl<T: AsyncRead + Debug + Send + Sync> AsyncReadDebugSendSync for T {}

impl<R: AsyncRead + Unpin + Debug + Send + Sync + 'static> Response<R> {
    pub fn into_boxed(self: Self) -> Response<Box<dyn AsyncReadDebugSendSync + Unpin>> {
        Response {
            headers: self.headers,
            status_code: self.status_code,
            body: Box::new(self.body),
        }
    }
}

impl From<String> for Response<Cursor<String>> {
    fn from(string: String) -> Self {
        Response {
            headers: Headers::from([
                (HeaderName::from("Content-Type"), "text/plain".to_string()),
                (HeaderName::from("Content-Length"), string.len().to_string()),
            ]),
            status_code: 200,
            body: Cursor::new(string),
        }
    }
}

impl<'a> From<u32> for Response<&'a [u8]> {
    fn from(code: u32) -> Self {
        let str = code.to_status_text();
        Response {
            headers: Headers::from([
                (HeaderName::from("Content-Type"), "text/plain".to_string()),
                (HeaderName::from("Content-Length"), str.len().to_string()),
            ]),
            status_code: code,
            body: str.as_bytes(),
        }
    }
}

#[derive(Error, Debug)]
pub enum ParsingError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    #[error("Client sent an empty request")]
    NullRequest,
    #[error("Reached EOF while reading headers")]
    Interrupted,
    #[error("Invalid first line")]
    InvalidFirstLine,
    #[error("Path longer than {0} bytes ({1} bytes)")]
    PathTooLong(usize, usize),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

#[derive(Error, Debug)]
pub enum HTTPError {
    #[error("Error while writing to socket: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Error while parsing request: {0}")]
    ParsingError(#[from] ParsingError),
    #[error("Error while handling request")]
    HandlingError,
}

async fn parse_http<'a, R: AsyncRead + AsyncBufRead + Unpin + Debug>(
    stream: R,
) -> Result<Request<R>, ParsingError> {
    let mut headers: Headers = Headers::with_capacity(32);

    let mut limited_stream = stream.take(MAX_HEADER_SIZE);
    let mut lines: Vec<String> = Vec::new();
    loop {
        let mut line = String::new();
        match limited_stream.read_line(&mut line).await {
            Ok(_) => {}
            Err(error) => {
                debug!("{:#?}", error);
                return match error.kind() {
                    ErrorKind::Interrupted => Err(ParsingError::Interrupted),
                    _ => Err(ParsingError::IOError(error)),
                };
            }
        };

        // .read_line() writes an empty string if it reaches EOF
        if line.is_empty() {
            return Err(ParsingError::Interrupted);
        }

        // Trim CRLF
        line = line.trim_end_matches(&['\r', '\n']).to_string();

        // Exit if we get double CRLF
        if line.is_empty() {
            // ..unless it's the first line, then fail
            if lines.len() == 0 {
                return Err(ParsingError::NullRequest);
            }
            break;
        } else {
            lines.push(line);
        }
    }

    let mut lines = lines.iter();

    // Parse first line
    let tokens: Vec<&str> = lines.next().unwrap().split(' ').collect();
    let len = tokens.len();
    if len != 3 {
        return Err(ParsingError::InvalidFirstLine);
    }

    let method = tokens[0].to_string();
    if method.is_empty() {
        return Err(ParsingError::InvalidFirstLine);
    }

    let mut path_param_split = tokens[1].splitn(2, '?');
    let (path, params) = match path_param_split.next() {
        Some(path) => (
            path.to_string(),
            match path_param_split.next() {
                Some(params) => Some(params.to_string()),
                None => None,
            },
        ),
        // Trust me
        None => unsafe { unreachable_unchecked() },
    };
    if path.len() > MAX_PATH_LEN {
        return Err(ParsingError::PathTooLong(MAX_PATH_LEN, path.len()));
    }
    if path.is_empty() || path.chars().nth(0).unwrap() != '/' {
        return Err(ParsingError::InvalidPath(path.to_string()));
    }

    loop {
        match lines.next() {
            Some(header) => {
                if let Some((name, value)) = header
                    .split_once(':')
                    .map(|(name, value)| (name.trim(), value.trim()))
                && !name.is_empty() {
                    headers.insert(HeaderName::from(name), value.to_string());
                } else {
                    debug!("Ignoring invalid header: {}", header);
                }
            }
            None => break
        }
    }
    return Ok(Request {
        headers,
        method,
        path,
        params,
        body: limited_stream.into_inner(),
    });
}

async fn write_response<R: AsyncRead + Unpin + Debug, W: AsyncWrite + Unpin>(
    mut response: Response<R>,
    mut stream: W,
) -> Result<(), std::io::Error> {
    stream.write("HTTP/1.1 ".as_bytes()).await?;
    stream
        .write(response.status_code.to_status_text().as_bytes())
        .await?;
    stream.write("\r\n".as_bytes()).await?;

    for (name, value) in response.headers {
        stream.write(name.to_string().as_bytes()).await?;
        stream.write(": ".as_bytes()).await?;
        stream.write(value.as_bytes()).await?;
        stream.write("\r\n".as_bytes()).await?;
    }

    stream.write("\r\n".as_bytes()).await?;

    tokio::io::copy(&mut response.body, &mut stream).await?;

    stream.flush().await?;

    Ok(())
}

#[derive(Debug, Clone, Copy)]
pub struct App {}

impl App {
    pub fn new() -> Self {
        Self {}
    }

    pub async fn listen<A: ToSocketAddrs>(self: Self, address: A) -> Result<(), JoinError> {
        let socket = TcpListener::bind(address).await.unwrap();

        let local_addr = socket.local_addr().unwrap();

        info!(
            "Listening on http://{}:{}",
            local_addr.ip().to_string().cyan().bold(),
            local_addr.port().to_string().red().bold()
        );

        let app = Arc::new(RwLock::new(self));
        while let Ok((stream, address)) = socket.accept().await {
            let app = app.clone();
            debug!("Established connection with {:#?}", address);
            task::spawn(async move {
                let app = app.read().await;
                let request_start = Instant::now();
                match app.handle_connection(stream, address).await {
                    Ok(str) => info!("({:#?}) {} ({:#?})", address, str, request_start.elapsed()),
                    // TODO: Actual error logging
                    Err(error) => error!(
                        "({:#?}) {} ({:#?})",
                        address,
                        error,
                        request_start.elapsed()
                    ),
                };
            })
            .await?;
            debug!("Connection with {:#?} ended", address);
        }
        Ok(())
    }

    async fn handle_request<R: AsyncRead + Unpin + Debug>(
        self: Self,
        request: Request<R>,
        address: SocketAddr,
    ) -> Result<Response<impl AsyncRead + Debug + Unpin>, ()> {
        if let Some(params) = request.params {
            return Ok(Response::from(format!("hello, {}\n", params)).into_boxed());
        } else {
            return Ok(Response::from(200).into_boxed());
        }
    }

    async fn handle_connection(
        self: Self,
        stream: TcpStream,
        address: SocketAddr,
    ) -> Result<String, HTTPError> {
        let (read_stream, write_stream) = tokio::io::split(stream);
        let request = parse_http(BufReader::new(read_stream)).await?;
        let path = request.path.clone();
        let method = request.method.clone();
        trace!("({:#?}) Handling request: {:#?}", address, request);
        match self.handle_request(request, address).await {
            Ok(response) => {
                let status_code = response.status_code;

                trace!("({:#?}) Sending response: {:#?}", address, response);
                write_response(response, BufWriter::new(write_stream)).await?;
                return Ok(format!(
                    "{} {} {}",
                    color_status_code(status_code).bold(),
                    method.bold(),
                    path
                ));
            }
            Err(_) => {
                return Err(HTTPError::HandlingError);
            }
        }
    }
}
