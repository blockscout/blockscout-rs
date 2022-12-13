use entity::sea_orm_active_enums::BytecodeType;
use eth_bytecode_db::{
    search::{find_partial_match_contracts, BytecodeRemote},
    tests::verifier_mock::{
        generate_and_insert, BytecodePart, ContractType, PartTy, VerificationResult,
    },
};
use migration::{DbErr, MigratorTrait};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
use std::{collections::HashMap, str::FromStr};
use url::Url;

pub struct TestDbGuard {
    conn: DatabaseConnection,
}

impl TestDbGuard {
    pub async fn new(db_name: &str) -> Self {
        let db_url = std::env::var("DATABASE_URL").expect("no DATABASE_URL env");
        let url = Url::parse(&db_url).expect("unvalid database url");
        let db_url = url.join("/").unwrap().to_string();
        let no_db_conn = Database::connect(db_url)
            .await
            .expect("failed to connect to postgres");

        Self::drop_database(&no_db_conn, db_name)
            .await
            .expect("cannot drop test database");
        Self::create_database(&no_db_conn, db_name)
            .await
            .expect("failed to create test db");

        let db_url = url.join(&format!("/{db_name}")).unwrap().to_string();
        let conn = Database::connect(db_url.clone())
            .await
            .expect("failed to connect to test db");
        Self::run_migrations(&conn)
            .await
            .expect("failed to migrate test db");
        TestDbGuard { conn }
    }

    pub async fn conn(&self) -> &DatabaseConnection {
        &self.conn
    }

    async fn drop_database(no_db_conn: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "dropping database");
        no_db_conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", db_name),
            ))
            .await?;
        Ok(())
    }

    async fn create_database(no_db_conn: &DatabaseConnection, db_name: &str) -> Result<(), DbErr> {
        tracing::info!(name = db_name, "creating database");
        no_db_conn
            .execute(Statement::from_string(
                sea_orm::DatabaseBackend::Postgres,
                format!("CREATE DATABASE {}", db_name),
            ))
            .await?;
        Ok(())
    }

    async fn run_migrations(conn: &DatabaseConnection) -> Result<(), DbErr> {
        <migration::Migrator as MigratorTrait>::up(conn, None).await
    }
}

#[tokio::test]
#[ignore]
async fn test_search_bytecodes() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("sqlx=warn".parse().unwrap()),
        )
        .init();
    let db = TestDbGuard::new("test_db_search_bytecodes").await;
    let conn = db.conn().await;
    let mut all_sources = HashMap::new();
    for i in 1..10 {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let source = generate_and_insert(conn, i, ty)
                .await
                .expect("cannot push contract");
            all_sources.insert((i, ty), source);
        }
    }

    let repeated_id = 77777;
    let repeated_amount = 10;
    let repeated_ty = ContractType::Small;
    for _ in 0..repeated_amount {
        let source = generate_and_insert(conn, repeated_id, repeated_ty)
            .await
            .expect("cannot push contract");
        all_sources.insert((repeated_id, repeated_ty), source);
    }

    // Search known bytecodes
    for i in 1..10 {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let expected_source = all_sources
                .get(&(i, ty))
                .expect("source should be in hashmap");
            let expected_contract = VerificationResult::generate(i, ty);

            let mut raw_creation_input = expected_contract
                .local_creation_input_parts
                .iter()
                .map(change_part_for_search)
                .collect::<Vec<_>>()
                .join("");

            match &expected_contract.constructor_arguments {
                Some(args) => raw_creation_input.push_str(args.trim_start_matches("0x")),
                None => {}
            };

            let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
                .unwrap()
                .0;
            let search = BytecodeRemote {
                data,
                bytecode_type: BytecodeType::CreationInput,
            };
            let partial_matches = find_partial_match_contracts(conn, &search)
                .await
                .expect("error during contract search");

            assert_eq!(
                partial_matches.len(),
                1,
                "contract not found. id={}, ty={:?}",
                i,
                ty
            );
            let contract = partial_matches
                .into_iter()
                .next()
                .expect("checked that len is 1");

            assert_eq!(&contract.contract_name, &expected_source.contract_name);
            assert_eq!(
                contract.constructor_arguments,
                expected_contract
                    .constructor_arguments
                    .map(|args| args.trim_start_matches("0x").to_string())
            );
        }
    }

    // Search repeated bytecodes

    let expected_source = all_sources
        .get(&(repeated_id, repeated_ty))
        .expect("source should be in hashmap");
    let expected_contract = VerificationResult::generate(repeated_id, repeated_ty);
    let mut raw_creation_input = expected_contract
        .local_creation_input_parts
        .iter()
        .map(change_part_for_search)
        .collect::<Vec<_>>()
        .join("");

    match &expected_contract.constructor_arguments {
        Some(args) => raw_creation_input.push_str(args.trim_start_matches("0x")),
        None => {}
    };

    let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
        .unwrap()
        .0;
    let search = BytecodeRemote {
        data,
        bytecode_type: BytecodeType::CreationInput,
    };
    let partial_matches = find_partial_match_contracts(conn, &search)
        .await
        .expect("error during contract search");
    assert_eq!(partial_matches.len(), repeated_amount);
    for contract in partial_matches {
        assert_eq!(&contract.contract_name, &expected_source.contract_name);
        assert_eq!(
            contract.constructor_arguments,
            expected_contract
                .clone()
                .constructor_arguments
                .map(|args| args.trim_start_matches("0x").to_string())
        );
    }

    // Search unknow bytecodes
    for i in 20..30 {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let unknow_contract = VerificationResult::generate(i, ty);
            let raw_creation_input = unknow_contract
                .local_creation_input_parts
                .iter()
                .map(change_part_for_search)
                .collect::<Vec<_>>()
                .join("");
            let data = blockscout_display_bytes::Bytes::from_str(&raw_creation_input)
                .unwrap()
                .0;
            let search = BytecodeRemote {
                data,
                bytecode_type: BytecodeType::CreationInput,
            };

            let partial_matches = find_partial_match_contracts(conn, &search)
                .await
                .expect("unkown contract should not give error");
            assert!(
                partial_matches.is_empty(),
                "found some contact, but bytecode is unknow"
            );
        }
    }

    // Search random strings
    for bytecode in ["", "6080", "0000", "1111"] {
        let data = blockscout_display_bytes::Bytes::from_str(bytecode)
            .unwrap()
            .0;
        let search = BytecodeRemote {
            data,
            bytecode_type: BytecodeType::CreationInput,
        };

        let partial_matches = find_partial_match_contracts(conn, &search)
            .await
            .expect("random string should not give error");
        assert!(
            partial_matches.is_empty(),
            "found some contact, but bytecode is random string"
        );
    }
}

fn change_part_for_search(part: &BytecodePart) -> String {
    let changed = match part.r#type {
        PartTy::Main => &part.data,
        PartTy::Meta => {
            let n = part.data.len();
            let metadata_length = &part.data[n - 4..];
            match metadata_length {
                "0033" => "a2646970667358221220c424331e61ba143d01f757e1a3b6ddcfe99698f6c1862e2133c4d7d277854b9564736f6c63430008070033",
                "0032" => "a265627a7a72315820a648f0e3107b949c9f7567adacfd4b276c9fc37dc06b172c7efbd1a0e58206ce64736f6c63430005110032",
                "0029" => "a165627a7a72305820a61b515152276dcea013aa8566142e7d3f07992c7c9512373cc7ba9a33fc2eab0029",
                _ => panic!("unknown metadata length '{}', add this type of metadata to mock", metadata_length)
            }
        }
    };
    changed.trim_start_matches("0x").to_string()
}
