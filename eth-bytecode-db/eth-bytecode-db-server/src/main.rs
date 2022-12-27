use eth_bytecode_db_server::Settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::new().expect("failed to read config");
    eth_bytecode_db_server::run(settings).await
}
