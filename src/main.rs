use bingus_http::{cool_macro, App, Request};
use log::LevelFilter;

async fn log_request(_request: Request<()>) -> anyhow::Result<u32> {
    Ok(418)
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(LevelFilter::Info)
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    let app = App::new(()).add_handler(cool_macro!(GET /), log_request);

    app.listen(address).await.unwrap();
}
