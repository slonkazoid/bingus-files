use crate::{
    handler::Handler,
    header::{HeaderName, Headers},
    method::{InvalidHeaderError, Method},
    request::{Params, Request},
    response::Response,
    route::{match_route, RouteToken},
    status::{color_status_code, StatusText},
    Route,
};
use colored::Colorize;
use log::{debug, error, info, trace};
use std::{collections::BTreeMap, sync::Arc};
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
const MAX_PATH_LEN: usize = 2048;

#[derive(Debug)]
pub struct HTTPRequest {
    pub headers: Headers,
    pub method: Method,
    pub path: String,
    pub query: Option<String>,
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
    #[error(transparent)]
    InvalidMethod(#[from] InvalidHeaderError),
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

    let method: Method = match tokens[0].try_into() {
        Ok(method) => method,
        Err(error) => return Err(ParsingError::InvalidMethod(error)),
    };

    let mut path_query_split = tokens[1].splitn(2, '?');
    let (path, query) = match path_query_split.next() {
        Some(path) => (
            path.to_string(),
            path_query_split.next().map(|params| params.to_string()),
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
        query,
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
    routes: BTreeMap<Route, Box<dyn Handler<S>>>,
}

impl<S> App<S>
where
    S: Clone + Debug + Send + Sync + 'static,
{
    pub fn new(state: S) -> Self {
        Self {
            state,
            routes: BTreeMap::new(),
        }
    }

    pub fn add_handler(mut self, route: Route, handler: impl Handler<S>) -> Self {
        self.routes.insert(route, Box::new(handler));
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
        let path: Vec<&str> = request.path.trim_matches('/').split('/').collect();
        if let Some((matched_route, _, matched_params, _)) =
            match_route(request.method.clone(), path.clone(), self.routes.keys())
        {
            trace!(
                "({:#?}) route matching path {:?} is {:?}",
                address,
                request.path,
                matched_route,
            );

            let mut params = Params::with_capacity(matched_params);

            if matched_params > 0 {
                for (index, token) in matched_route.1.iter().enumerate() {
                    match token {
                        RouteToken::PARAMETER(param) => {
                            params.insert(param.to_string(), path[index].to_string());
                        }
                        _ => {}
                    }
                }
            }

            let handler = self
                .routes
                .get(matched_route)
                .unwrap_or_else(|| unsafe { unreachable_unchecked() });

            return handler
                .call(Request {
                    state: self.state.clone(),
                    address,
                    request,
                    params,
                })
                .await;
        }

        trace!("({:#?}), no routes matches path {}", address, request.path);
        Ok(Response::default())
    }

    async fn handle_connection(
        &self,
        stream: TcpStream,
        address: SocketAddr,
    ) -> Result<String, HTTPError> {
        let (read_stream, write_stream) = tokio::io::split(stream);
        let request = parse_http(BufReader::new(read_stream)).await?;
        let path = request.path.clone();
        let method: &str = request.method.clone().into();
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
