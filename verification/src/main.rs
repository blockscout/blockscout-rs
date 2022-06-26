use verification::{run_http_server, Args, Config};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    let args = Args::default();
    let config = Config::from_file(args.config_path).expect("Failed to parse config");
    run_http_server(config).await
}
