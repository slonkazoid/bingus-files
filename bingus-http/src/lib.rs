#![feature(let_chains, iter_intersperse, async_closure, fn_traits)]

extern crate bingus_http_proc_macro;

pub mod handler;
pub mod header;
pub mod http;
pub mod method;
pub mod request;
pub mod response;
pub mod route;
pub mod status;

pub use crate::bingus_http_proc_macro::cool_macro;
pub use crate::http::App;
pub use crate::method::Method;
pub use crate::request::Request;
pub use crate::response::Response;
pub use crate::route::Route;
