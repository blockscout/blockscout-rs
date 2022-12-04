use entity::sea_orm_active_enums::BytecodeType;
use eth_bytecode_db::{
    search::{find_partial_match_contract, BytecodeRemote},
    tests::verifier_mock::{ContractType, VerificationResult},
};
use sea_orm::{Database, DatabaseConnection};
use std::str::FromStr;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db_url = std::env::var_os("DATABASE_URL")
        .map(|v| v.into_string().unwrap())
        .expect("no DATABASE_URL env");
    let db: DatabaseConnection = Database::connect(db_url).await.unwrap();
    let n = 10;
    let now = std::time::Instant::now();
    for i in 0..n {
        let raw_creation_input = VerificationResult::generate(10 + i, ContractType::Small)
            .local_creation_input_parts
            .iter()
            .map(|p| p.data.trim_start_matches("0x"))
            .collect::<Vec<_>>()
            .join("");
        let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
            .unwrap()
            .0;
        let search = BytecodeRemote {
            data,
            bytecode_type: BytecodeType::CreationInput,
        };
        let partial_match = find_partial_match_contract(&db, search).await;
        println!("{:?}", partial_match);
    }
    println!("AVG time: {}", now.elapsed().as_secs_f64() / (n as f64));
}
