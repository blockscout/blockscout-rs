use entity::sources;
use eth_bytecode_db::tests::verifier_mock::{generate_and_insert, ContractInfo, ContractType};
use sea_orm::{Database, DatabaseConnection, EntityTrait, PaginatorTrait};
use std::sync::Arc;
use tokio::sync::Semaphore;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let db_url = std::env::var("DATABASE_URL").expect("no DATABASE_URL env");
    let db: DatabaseConnection = Database::connect(db_url).await.unwrap();
    let count = sources::Entity::find().count(&db).await.unwrap();
    if count < 10000 {
        let semaphore = Arc::new(Semaphore::new(10));
        let db = Arc::new(db);
        let mut join_handles = Vec::new();

        for i in 0..1000 {
            if i % 100 == 0 {
                tracing::info!("SAME CONTRACTS. task #{}", i);
            }

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db = db.clone();
            let info = ContractInfo {
                id: 1,
                ty: ContractType::Small,
            };
            join_handles.push(tokio::spawn(async move {
                let res = generate_and_insert(db.as_ref(), &info).await;
                drop(permit);
                res
            }));
        }

        for id in 10..5020 {
            if id % 100 == 0 {
                tracing::info!("DIFFERENT SMALL CONTRACTS. task #{}", id);
            }

            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db = db.clone();
            let info = ContractInfo {
                id,
                ty: ContractType::Small,
            };
            join_handles.push(tokio::spawn(async move {
                let res = generate_and_insert(db.as_ref(), &info).await;
                drop(permit);
                res
            }));
        }

        for id in 10..5020 {
            if id % 100 == 0 {
                tracing::info!("DIFFERENT MEDIUM CONTRACT. task #{}", id);
            }
            let permit = semaphore.clone().acquire_owned().await.unwrap();
            let db = db.clone();
            let info = ContractInfo {
                id,
                ty: ContractType::Medium,
            };
            join_handles.push(tokio::spawn(async move {
                let res = generate_and_insert(db.as_ref(), &info).await;
                drop(permit);
                res
            }));
        }

        for handle in join_handles {
            handle.await.unwrap().unwrap();
        }
    } else {
        println!("Database is full already. exit");
    }
}
