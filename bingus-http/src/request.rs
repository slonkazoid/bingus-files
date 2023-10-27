use crate::http::HTTPRequest;
use std::{net::SocketAddr, collections::HashMap};

pub type Params = HashMap<String, String>;

#[derive(Debug)]
pub struct Request<S> {
    pub state: S,
    pub address: SocketAddr,
    pub request: HTTPRequest,
    pub params: Params
}
