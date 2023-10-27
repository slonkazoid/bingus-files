use bingus_http::{cool_macro, App, Request};

use log::{info, Level};

async fn log_request(request: Request<()>) -> anyhow::Result<u32> {
    info!("{:#?}", request.params);
    Ok(200)
}

#[tokio::main]
async fn main() {
    env_logger::builder()
        .filter_level(Level::Info.to_level_filter())
        .parse_default_env()
        .init();

    let address = "0.0.0.0:4040";

    let state = ();

    let app = App::new(state)
        .add_handler(cool_macro!(GET / debug), log_request)
        .add_handler(cool_macro!(GET / debug / :var), log_request)
        .add_handler(cool_macro!(GET / debug / :var1 / :var2), log_request)
        .add_handler(cool_macro!(GET / debug / :var1 / hi / :var2), log_request);

    app.listen(address).await.unwrap();
}
