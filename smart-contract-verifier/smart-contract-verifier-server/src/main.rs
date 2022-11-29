use smart_contract_verifier_server::Settings;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    let settings = Settings::new().expect("failed to read config");
    smart_contract_verifier_server::run(settings).await
}
