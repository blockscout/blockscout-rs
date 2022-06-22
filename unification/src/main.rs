use unification::config;
use unification::run;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let settings = config::get_config().expect("Failed to parse config");
    run(settings)?.await
}
