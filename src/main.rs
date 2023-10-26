use bingus_http::{cool_macro, App, Request, Response};

use log::{info, Level};
use std::sync::{Arc, Mutex};

#[derive(Debug, Default)]
struct State {
    requests: u64,
}

async fn hello(request: Request<Arc<Mutex<State>>>) -> anyhow::Result<String> {
    Ok(format!(
        "Hi, {:#?}. The counter is at {}.\n",
        request.address.ip(),
        (match request.state.lock() {
            Ok(v) => v,
            Err(e) => e.into_inner(),
        })
        .requests
    ))
}

#[allow(dead_code)]
async fn increment(request: Request<Arc<Mutex<State>>>) -> anyhow::Result<String> {
    let mut lock = match request.state.lock() {
        Ok(v) => v,
        Err(_) => return Ok("Oops, the counter broke.\n".to_string()),
    };
    lock.requests += 1;
    Ok(format!("Incremented the counter!\n"))
}

#[allow(dead_code)]
async fn trole(request: Request<Arc<Mutex<State>>>) -> anyhow::Result<Response> {
    info!("{:#?}", request);
    Ok(Response::default())
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Level::Info.to_level_filter())
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    let state = Arc::new(Mutex::new(State { requests: 0 }));

    let app = App::new(state)
        .add_handler(cool_macro!(GET /), hello)
        .add_handler(cool_macro!(GET / hello), hello)
        .add_handler(cool_macro!(GET /hello/:var), hello)
        .add_handler(cool_macro!(GET /:var1/:var2), hello)
        .add_handler(cool_macro!(GET /:var/hi), hello)
        .add_handler(cool_macro!(GET / hello / hi), hello)
        .add_handler(cool_macro!(GET /hello/ *), hello)
        .add_handler(cool_macro!(GET /:var), hello)
        .add_handler(cool_macro!(GET /:var/ *), hello)
        .add_handler(cool_macro!(GET / *), hello);

    app.listen(address).await.unwrap();
}
