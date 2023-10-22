use crate::status::to_status_text;
use colored::{ColoredString, Colorize};
use log::{trace, warn};
use std::{collections::HashMap, fmt::Debug, io::ErrorKind, net::SocketAddr};
use thiserror::Error;
use tokio::{
    io::{
        AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt,
        BufReader, BufWriter,
    },
    net::TcpStream,
};

const MAX_HEADER_SIZE: u64 = 8192;

#[derive(Debug, Clone)]
pub enum Method {
    GET,
    POST,
}

impl<'a> Into<&'a str> for Method {
    fn into(self: Self) -> &'a str {
        match self {
            Method::GET => "GET",
            Method::POST => "POST",
        }
    }
}

impl<'a> TryFrom<&'a str> for Method {
    type Error = String;

    fn try_from(string: &'a str) -> Result<Self, Self::Error> {
        match string {
            "GET" => Ok(Method::GET),
            "POST" => Ok(Method::POST),
            _ => Err(format!("Not a valid method: {}", string)),
        }
    }
}

type Headers = HashMap<String, String>;

#[derive(Debug)]
pub struct Request<R: AsyncRead> {
    pub headers: Headers,
    pub method: Method,
    pub path: String,
    pub body: R,
}

pub struct Response<R: AsyncRead> {
    pub headers: Headers,
    pub status_code: u32,
    pub body: R,
}

impl<'a> From<&'a str> for Response<&'a [u8]> {
    fn from(str: &'a str) -> Self {
        Response {
            headers: Headers::from([
                ("Content-Type".to_string(), "text/plain".to_string()),
                ("Content-Length".to_string(), str.len().to_string()),
            ]),
            status_code: 200,
            body: str.as_bytes(),
        }
    }
}

impl<'a> From<u32> for Response<&'a [u8]> {
    fn from(code: u32) -> Self {
        let status_text = to_status_text(code);
        Response {
            headers: Headers::from([
                ("Content-Type".to_string(), "text/plain".to_string()),
                ("Content-Length".to_string(), status_text.len().to_string()),
            ]),
            status_code: code,
            body: status_text.as_bytes(),
        }
    }
}

#[derive(Error, Debug)]
pub enum ParsingError {
    #[error(transparent)]
    IOError(#[from] std::io::Error),
    // TODO: ~~Do not do this~~
    // TODO: Refactor every instance where GenericParsingError is used
    #[error("{0}")]
    GenericParsingError(String),
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

fn color_status_code(status_code: u32) -> ColoredString {
    match status_code {
        100..=199 => status_code.to_string().white(),
        200..=299 => status_code.to_string().bright_green(),
        300..=399 => status_code.to_string().yellow(),
        400..=499 => status_code.to_string().bright_red(),
        500..=599 => status_code.to_string().red(),
        _ => status_code.to_string().normal(),
    }
}

async fn parse_http<R: AsyncRead + AsyncBufRead + Unpin>(
    stream: R,
) -> Result<Request<R>, ParsingError> {
    let headers: HashMap<String, String> = HashMap::with_capacity(32);
    let method: Method;
    let path: &str;

    let mut limited_stream = stream.take(MAX_HEADER_SIZE);
    let mut lines: Vec<String> = Vec::new();
    loop {
        let mut line = String::new();
        match limited_stream.read_line(&mut line).await {
            Ok(_) => {}
            Err(error) => {
                return match error.kind() {
                    ErrorKind::Interrupted => Err(ParsingError::GenericParsingError(format!(
                        "Stupid fucking client"
                    ))),
                    _ => Err(ParsingError::IOError(error)),
                }
            }
        };

        line = line.trim_end_matches(&['\r', '\n']).to_string();

        if line.is_empty() {
            break;
        } else {
            lines.push(line);
        }
    }

    // Parse first line
    let tokens: Vec<&str> = lines[0].split(' ').collect();
    let len = tokens.len();
    if len != 3 {
        return Err(ParsingError::GenericParsingError(format!(
            "First line has {} tokens",
            len
        )));
    }

    method = match tokens[0].try_into() {
        Ok(method) => method,
        Err(error) => return Err(ParsingError::GenericParsingError(error.to_string())),
    };

    path = tokens[1];
    if path.len() > 2048 {
        return Err(ParsingError::GenericParsingError(
            "Path longer than 2048 characters".to_string(),
        ));
    }
    if path.is_empty() {
        return Err(ParsingError::GenericParsingError(format!("Path is empty")));
    }
    if path.chars().nth(0).unwrap() != '/' {
        return Err(ParsingError::GenericParsingError(format!(
            "Invalid path: {}",
            path
        )));
    }

    return Ok(Request {
        headers,
        method,
        path: path.to_string(),
        body: limited_stream.into_inner(),
    });
}

async fn handle_request<'a, R: AsyncRead + Debug>(
    request: Request<R>,
    address: SocketAddr,
) -> Result<Response<&'a [u8]>, ()> {
    trace!("({:#?}) Handling request: {:#?}", address, request);
    Ok(Response::from("hello\n"))
}

async fn write_response<R: AsyncRead + Unpin, W: AsyncWrite + Unpin>(
    mut response: Response<R>,
    mut stream: W,
) -> Result<(), std::io::Error> {
    stream.write("HTTP/1.1 ".as_bytes()).await?;
    stream
        .write(to_status_text(response.status_code).as_bytes())
        .await?;
    stream.write("\r\n".as_bytes()).await?;

    for (header, value) in response.headers {
        stream.write(header.as_bytes()).await?;
        stream.write(": ".as_bytes()).await?;
        stream.write(value.as_bytes()).await?;
        stream.write("\r\n".as_bytes()).await?;
    }

    stream.write("\r\n".as_bytes()).await?;

    tokio::io::copy(&mut response.body, &mut stream).await?;

    stream.flush().await?;

    Ok(())
}

pub async fn handle_connection(
    stream: TcpStream,
    address: SocketAddr,
) -> Result<String, HTTPError> {
    let (read_stream, write_stream) = tokio::io::split(stream);
    let request = parse_http(BufReader::new(read_stream)).await?;
    let path = request.path.clone();
    let method: &str = request.method.clone().into();
    match handle_request(request, address).await {
        Ok(response) => {
            let status_code = response.status_code;
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
