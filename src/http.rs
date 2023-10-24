use crate::{
    handler::Handler,
    header::{HeaderName, Headers},
    request::Request,
    response::Response,
    status::{color_status_code, StatusText},
};
use colored::Colorize;
use log::{debug, error, info, trace};
use std::sync::Arc;
use std::{fmt::Debug, hint::unreachable_unchecked, io::ErrorKind, net::SocketAddr};
use thiserror::Error;
use tokio::{
    io::{
        AsyncBufReadExt, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader, BufWriter, ReadHalf,
    },
    net::{TcpListener, TcpStream, ToSocketAddrs},
    task::{self, JoinError},
    time::Instant,
};

const MAX_HEADER_SIZE: u64 = 8192;
const MAX_METHOD_LEN: usize = 16;
const MAX_PATH_LEN: usize = 2048;

#[derive(Debug)]
pub struct HTTPRequest {
    pub headers: Headers,
    pub method: String,
    pub path: String,
    pub params: Option<String>,
    pub body: ReadHalf<TcpStream>,
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
    #[error("Method longer than {0} bytes ({1} bytes)")]
    MethodTooLong(usize, usize),
    #[error("Invalid method: {0}")]
    InvalidMethod(String),
    #[error("Path longer than {0} bytes ({1} bytes)")]
    PathTooLong(usize, usize),
    #[error("Invalid path: {0}")]
    InvalidPath(String),
}

#[allow(clippy::enum_variant_names)]
#[derive(Error, Debug)]
pub enum HTTPError {
    #[error("Error while writing to socket: {0}")]
    IOError(#[from] std::io::Error),
    #[error("Error while parsing request: {0}")]
    ParsingError(#[from] ParsingError),
    #[error("Error while handling request: {0}")]
    HandlingError(#[from] anyhow::Error),
}

async fn parse_http<'a>(
    stream: BufReader<ReadHalf<TcpStream>>,
) -> Result<HTTPRequest, ParsingError> {
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
            if lines.is_empty() {
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
    if method.len() > MAX_METHOD_LEN {
        return Err(ParsingError::MethodTooLong(MAX_METHOD_LEN, method.len()));
    }
    if method.is_empty() {
        return Err(ParsingError::InvalidMethod(method));
    }

    let mut path_param_split = tokens[1].splitn(2, '?');
    let (path, params) = match path_param_split.next() {
        Some(path) => (
            path.to_string(),
            path_param_split.next().map(|params| params.to_string()),
        ),
        // Trust me
        None => unsafe { unreachable_unchecked() },
    };
    if path.len() > MAX_PATH_LEN {
        return Err(ParsingError::PathTooLong(MAX_PATH_LEN, path.len()));
    }
    if path.is_empty() || !path.starts_with('/') {
        return Err(ParsingError::InvalidPath(path.to_string()));
    }

    for header in lines {
        if let Some((name, value)) = header
                    .split_once(':')
                    .map(|(name, value)| (name.trim(), value.trim()))
                && !name.is_empty() {
                    headers.insert(HeaderName::from(name), value.to_string());
                } else {
                    debug!("Ignoring invalid header: {}", header);
                }
    }
    Ok(HTTPRequest {
        headers,
        method,
        path,
        params,
        body: limited_stream.into_inner().into_inner(),
    })
}

async fn write_response<W: AsyncWrite + Unpin>(
    mut response: Response,
    mut stream: W,
) -> Result<(), std::io::Error> {
    stream.write_all("HTTP/1.1 ".as_bytes()).await?;
    stream
        .write_all(response.status_code.to_status_text().as_bytes())
        .await?;
    stream.write_all("\r\n".as_bytes()).await?;

    for (name, value) in response.headers {
        stream.write_all(name.to_string().as_bytes()).await?;
        stream.write_all(": ".as_bytes()).await?;
        stream.write_all(value.as_bytes()).await?;
        stream.write_all("\r\n".as_bytes()).await?;
    }

    stream.write_all("\r\n".as_bytes()).await?;

    tokio::io::copy(&mut response.body, &mut stream).await?;

    stream.flush().await?;

    Ok(())
}

pub struct App<S>
where
    S: Clone + Debug + Send + Sync + 'static,
{
    state: S,
    handler: Vec<Box<dyn Handler<S>>>,
}

impl<S> App<S>
where
    S: Clone + Debug + Send + Sync + 'static,
{
    pub fn new(state: S) -> Self {
        Self {
            state,
            handler: Vec::new(),
        }
    }

    pub fn add_handler(mut self, handler: impl Handler<S>) -> Self {
        self.handler.push(Box::new(handler));
        self
    }

    pub async fn listen<A: ToSocketAddrs>(self, address: A) -> Result<(), JoinError> {
        let socket = TcpListener::bind(address).await.unwrap();

        let local_addr = socket.local_addr().unwrap();

        info!(
            "Listening on http://{}:{}",
            local_addr.ip().to_string().bold(),
            local_addr.port().to_string().cyan().bold()
        );

        let app = Arc::new(self);

        while let Ok((stream, address)) = socket.accept().await {
            let app = app.clone();
            debug!("Established connection with {:#?}", address);
            task::spawn(async move {
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

    async fn handle_request(
        &self,
        request: HTTPRequest,
        address: SocketAddr,
    ) -> anyhow::Result<Response> {
        match self.handler.first() {
            Some(h) => Ok(h
                .call(Request {
                    state: self.state.clone(),
                    address,
                    request,
                })
                .await?),
            None => todo!(),
        }
    }

    async fn handle_connection(
        &self,
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
                Ok(format!(
                    "{} {} {}",
                    color_status_code(status_code).bold(),
                    method.bold(),
                    path
                ))
            }
            Err(error) => Err(HTTPError::HandlingError(error)),
        }
    }
}
