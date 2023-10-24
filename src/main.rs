#![feature(let_chains, iter_intersperse, async_closure, fn_traits)]

mod header;
mod http;
mod status;

use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use env_logger;
use http::{App, Request, Response};
use log::Level;
use tokio::task;

#[derive(Debug, Clone)]
struct State {
    i: u64,
}

impl Default for State {
    fn default() -> Self {
        Self { i: 0u64 }
    }
}

async fn hello(
    request: Request,
    address: SocketAddr,
    state: Arc<Mutex<State>>,
) -> anyhow::Result<Response> {
    match request.path.as_str() {
        "/" => Ok(Response::from(format!(
            "Hi, {:#?}, the counter is at {}\n",
            address.ip(),
            state.lock().unwrap().i
        ))),
        _ => Ok(Response::from(format!("Meow\n"))),
    }
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Level::Info.to_level_filter())
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    let state = Arc::new(Mutex::new(State { i: 69u64 }));

    let app = App::new(state.clone(), hello);

    task::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            state.lock().unwrap().i += 1;
        }
    });

    app.listen(address).await.unwrap();
}
