use multichain_api_gateway::{config, run};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let settings = config::get_config();
    run(settings)?.await
}
