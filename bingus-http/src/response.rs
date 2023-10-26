use crate::{
    header::{HeaderName, Headers},
    status::StatusText,
};
use std::{fmt::Debug, io::Cursor};
use tokio::io::AsyncRead;

#[derive(Debug)]
pub struct Response {
    pub headers: Headers,
    pub status_code: u32,
    pub body: ResponseBody,
}

pub trait ResponseBodyTrait: AsyncRead + Debug + Send + Sync + Unpin {}
impl<T: AsyncRead + Debug + Send + Sync + Unpin> ResponseBodyTrait for T {}

pub type ResponseBody = Box<dyn ResponseBodyTrait>;

impl From<String> for Response {
    fn from(string: String) -> Self {
        Response {
            headers: Headers::from([
                (HeaderName::from("Content-Type"), "text/plain".to_string()),
                (HeaderName::from("Content-Length"), string.len().to_string()),
            ]),
            status_code: 200,
            body: Box::new(Cursor::new(string)),
        }
    }
}

impl From<u32> for Response {
    fn from(code: u32) -> Self {
        let string = format!("{}\n", code.to_status_text());
        Response {
            headers: Headers::from([
                (HeaderName::from("Content-Type"), "text/plain".to_string()),
                (HeaderName::from("Content-Length"), string.len().to_string()),
            ]),
            status_code: code,
            body: Box::new(Cursor::new(string)),
        }
    }
}

impl Default for Response {
    #[inline]
    fn default() -> Self {
        Self::from(200)
    }
}
