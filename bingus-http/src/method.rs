use thiserror::Error;

// TODO: proc_macro away all the boilerplate

#[derive(Debug, Error)]
#[error("Invalid header: {0}")]
pub struct InvalidHeaderError(pub String);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Method {
    GET,
    HEAD,
    POST,
    PUT,
    DELETE,
    CONNECT,
    OPTIONS,
    TRACE,
    PATCH,
}

impl<'a> From<Method> for &'a str {
    fn from(method: Method) -> Self {
        match method {
            Method::GET => "GET",
            Method::HEAD => "HEAD",
            Method::POST => "POST",
            Method::PUT => "PUT",
            Method::DELETE => "DELETE",
            Method::CONNECT => "CONNECT",
            Method::OPTIONS => "OPTIONS",
            Method::TRACE => "TRACE",
            Method::PATCH => "PATCH",
        }
    }
}

impl<'a> TryFrom<&'a str> for Method {
    type Error = InvalidHeaderError;

    fn try_from(string: &'a str) -> Result<Self, Self::Error> {
        match string {
            "GET" => Ok(Self::GET),
            "HEAD" => Ok(Self::HEAD),
            "POST" => Ok(Self::POST),
            "PUT" => Ok(Self::PUT),
            "DELETE" => Ok(Self::DELETE),
            "CONNECT" => Ok(Self::CONNECT),
            "OPTIONS" => Ok(Self::OPTIONS),
            "TRACE" => Ok(Self::TRACE),
            "PATCH" => Ok(Self::PATCH),
            _ => Err(InvalidHeaderError(string.to_string())),
        }
    }
}

impl TryFrom<String> for Method {
    type Error = InvalidHeaderError;

    #[inline]
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::try_from(value.as_str())
    }
}
