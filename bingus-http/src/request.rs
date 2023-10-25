use crate::http::HTTPRequest;
use std::net::SocketAddr;

#[derive(Debug)]
pub struct Request<S> {
    pub state: S,
    pub address: SocketAddr,
    pub request: HTTPRequest,
}
