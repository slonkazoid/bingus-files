#![feature(let_chains, iter_intersperse)]

mod header;
mod http;
mod status;

use env_logger;
use http::App;
use log::Level;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Level::Info.to_level_filter())
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    App::new().listen(address).await.unwrap();
}
