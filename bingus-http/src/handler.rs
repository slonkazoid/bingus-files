use crate::{request::Request, response::Response};
use async_trait::async_trait;
use std::{fmt::Debug, future::Future};

#[async_trait]
pub trait Handler<S>: Send + Sync + 'static
where
    S: Clone + Debug + Send + Sync + 'static,
{
    async fn call(&self, request: Request<S>) -> anyhow::Result<Response>;
}

#[async_trait]
impl<S, F, Fut, Res> Handler<S> for F
where
    S: Clone + Debug + Send + Sync + 'static,
    F: Send + Sync + 'static + Fn(Request<S>) -> Fut,
    Fut: Future<Output = anyhow::Result<Res>> + Send + 'static,
    Res: Into<Response> + 'static,
{
    async fn call(&self, request: Request<S>) -> anyhow::Result<Response> {
        let fut = (self)(request);
        let res = fut.await?;
        Ok(res.into())
    }
}
