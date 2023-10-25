use crate::method::Method;

#[derive(Debug)]
pub enum RouteToken {
    PATH(String),
    VARIABLE(String),
    WILDCARD,
}

#[derive(Debug)]
pub struct Route(pub Method, pub Vec<RouteToken>);
