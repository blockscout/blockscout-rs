use eth_bytecode_db::verification::import_existing_abis;
use sea_orm::Database;

#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").expect("DATABASE_URL env was not provided");
    let db_client = Database::connect(db_url)
        .await
        .expect("Error connecting to database");

    import_existing_abis::import(&db_client).await;
}
