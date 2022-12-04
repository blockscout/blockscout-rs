// use entity::{sea_orm_active_enums::BytecodeType, sources};
// use eth_bytecode_db::{
//     search::{find_partial_match_contract, BytecodeRemote}, tests::{verifier_mock::{VerificationResult, ContractType}, insert_verification::insert_verification_result},
// };
// use sea_orm::{Database, DatabaseConnection, EntityTrait, PaginatorTrait};
// use std::{str::FromStr, sync::Arc};
// use tokio::sync::Semaphore;

// #[tokio::main]
// async fn main() {
//     tracing_subscriber::fmt()
//         .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
//         .init();

//     let db_url = std::env::var_os("DATABASE_URL")
//         .map(|v| v.into_string().unwrap())
//         .expect("no DATABASE_URL env");
//     let db: DatabaseConnection = Database::connect(db_url).await.unwrap();
//     let count = sources::Entity::find().count(&db).await.unwrap();
//     if count < 10000 {
//         let semaphore = Arc::new(Semaphore::new(10));
//         let db = Arc::new(db);
//         let mut join_handles = Vec::new();

//         for i in 0..1000 {
//             if i % 100 == 0 {
//                 println!("SAME CONTRACTS. task #{}", i);
//             }

//             let permit = semaphore.clone().acquire_owned().await.unwrap();
//             let db = db.clone();
//             join_handles.push(tokio::spawn(async move {
//                 let res = push_contract(db, 1, 1).await;
//                 drop(permit);
//                 res
//             }));
//         }

//         for id in 10..5020 {
//             if id % 100 == 0 {
//                 println!("DIFFERENT SMALL CONTRACTS. task #{}", id);
//             }

//             let permit = semaphore.clone().acquire_owned().await.unwrap();
//             let db = db.clone();
//             join_handles.push(tokio::spawn(async move {
//                 let res = push_contract(db, id, 1).await;
//                 drop(permit);
//                 res
//             }));
//         }

//         for id in 10..5020 {
//             if id % 100 == 0 {
//                 println!("DIFFERENT BIG CONTRACT. task #{}", id);
//             }
//             let permit = semaphore.clone().acquire_owned().await.unwrap();
//             let db = db.clone();
//             join_handles.push(tokio::spawn(async move {
//                 let res = push_contract(db, id, 2).await;
//                 drop(permit);
//                 res
//             }));
//         }

//         for handle in join_handles {
//             handle.await.unwrap().unwrap();
//         }
//     } else {
//         println!("database is full already. search");
//         let n = 1;
//         let now = std::time::Instant::now();
//         for i in 0..n {
//             let raw_creation_input = get_contract(91 + i, 1)
//                 .local_creation_input_parts
//                 .iter()
//                 .map(|p| p.data.trim_start_matches("0x"))
//                 .collect::<Vec<_>>()
//                 .join("");
//             let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
//                 .unwrap()
//                 .0;
//             let search = BytecodeRemote {
//                 data,
//                 bytecode_type: BytecodeType::CreationInput,
//             };
//             let partial_match = find_partial_match_contract(&db, search).await;
//             println!("{:?}", partial_match);
//         }
//         println!("AVG time: {}", now.elapsed().as_secs_f64() / (n as f64));
//     }
// }

// async fn push_contract(db: Arc<DatabaseConnection>, id: usize, ty: ContractType) -> Result<(), anyhow::Error> {
//     let verification_result = VerificationResult::generate(id, ty);
//     println!("push contract {:?}/{}", ty, id);
//     insert_verification_result(db.as_ref(), verification_result).await?;
//     Ok(())
// }

fn main() {
    todo!()
}
