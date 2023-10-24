#![feature(let_chains, iter_intersperse, async_closure, fn_traits)]

mod handler;
mod header;
mod http;
mod request;
mod response;
mod status;

use http::App;
use request::Request;

use log::Level;
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct State {
    requests: u64,
}

async fn hello(request: Request<Arc<Mutex<State>>>) -> anyhow::Result<String> {
    request.state.lock().unwrap().requests += 1;
    Ok(format!(
        "Hi, {:#?}, the counter is at {}\n",
        request.address.ip(),
        request.state.lock().unwrap().requests
    ))
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Level::Info.to_level_filter())
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    let state = Arc::new(Mutex::new(State { requests: 0 }));

    let app = App::new(state).add_handler(hello);

    app.listen(address).await.unwrap();
}
