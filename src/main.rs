#![feature(let_chains)]

mod http;
mod status;

use crate::http::handle_connection;
use env_logger;

use colored::*;
use log::{debug, error, info, Level};
use tokio::net::TcpListener;
use tokio::task;
use tokio::time::Instant;

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Level::Info.to_level_filter())
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    let socket = TcpListener::bind(address).await.unwrap();

    let local_addr = socket.local_addr().unwrap();

    info!(
        "Listening on http://{}:{}",
        local_addr.ip().to_string().cyan().bold(),
        local_addr.port().to_string().red().bold()
    );

    while let Ok((stream, address)) = socket.accept().await {
        debug!("Established connection with {:#?}", address);
        let request_start = Instant::now();
        match task::spawn(handle_connection(stream, address))
            .await
            .unwrap()
        {
            Ok(str) => info!("({:#?}) {} ({:#?})", address, str, request_start.elapsed()),
            // TODO: Actual error logging
            Err(error) => error!(
                "({:#?}) {} ({:#?})",
                address,
                error,
                request_start.elapsed()
            ),
        };
        debug!("Connection with {:#?} ended", address);
    }
}
