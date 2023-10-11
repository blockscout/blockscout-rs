use visualizer_server::Settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::new().expect("failed to read config");
    visualizer_server::run(settings).await
}
