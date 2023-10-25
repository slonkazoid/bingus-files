use bingus_http::{path, App, Request};

use log::{info, Level};
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

    info!("{:?}", path!(GET /hi/:ix));

    app.listen(address).await.unwrap();
}
