use std::{collections::HashMap, str::FromStr};

use entity::{sea_orm_active_enums::BytecodeType, sources};
use eth_bytecode_db::{
    create::{create_source, BytecodePart, VerificationResult},
    search::{find_partial_match_contract, BytecodeRemote},
};
use migration::{DbErr, MigratorTrait};
use rstest::*;
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, Statement};
use url::Url;

pub struct TestDbGuard {
    conn: DatabaseConnection,
}

impl TestDbGuard {
    pub async fn new(db_name: &str) -> Self {
        let db_url = std::env::var_os("DATABASE_URL")
            .map(|v| v.into_string().unwrap())
            .expect("no DATABASE_URL env");
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

#[fixture]
async fn db(#[default("default")] name: &str) -> TestDbGuard {
    let db_name = format!("test_db_{}", name);
    TestDbGuard::new(&db_name).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum ContractType {
    Small,
    Medium,
    Big,
    Constructor,
}

fn get_contract(id: usize, ty: ContractType) -> VerificationResult {
    match ty {
        ContractType::Small => {
            let template = include_str!("contracts/type_1.json");
            get_verification_result(template, id).expect("should be valid verification result")
        }
        ContractType::Medium => {
            let template = include_str!("contracts/type_2.json");
            get_verification_result(template, id).expect("should be valid verification result")
        }
        ContractType::Big => {
            let template = include_str!("contracts/type_3.json");
            get_verification_result(template, id).expect("should be valid verification result")
        }
        ContractType::Constructor => {
            let template = include_str!("contracts/type_4.json");
            get_verification_result(template, id).expect("should be valid verification result")
        }
    }
}

fn get_verification_result(
    template: &str,
    id: usize,
) -> Result<VerificationResult, serde_json::Error> {
    serde_json::from_str(&template.replace("{{ID}}", &format!("{:0>10}", id)))
}

async fn push_contract(
    db: &DatabaseConnection,
    id: usize,
    ty: ContractType,
) -> Result<sources::Model, anyhow::Error> {
    let verification_result = get_contract(id, ty);
    create_source(db, verification_result).await
}

#[rstest::rstest]
#[tokio::test]
async fn test_search_bytecode(
    #[future]
    #[with("search_bytecode")]
    db: TestDbGuard,
) {
    tracing_subscriber::fmt()
        .with_env_filter("search=info,sqlx=warn")
        .init();

    let db = db.await;
    let conn = db.conn().await;
    let mut all_sources = HashMap::new();
    for i in 1..10 {
        for ty in [
            ContractType::Small,
            ContractType::Medium,
            ContractType::Big,
            ContractType::Constructor,
        ] {
            let source = push_contract(conn, i, ty)
                .await
                .expect("cannot push contract");
            all_sources.insert((i, ty), source);
        }
    }

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
            let expected_contract = get_contract(i, ty);

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
            let partial_match = find_partial_match_contract(conn, search).await;
            let contract = partial_match
                .expect("error during contract search")
                .unwrap_or_else(|| panic!("contract not found. id={}, ty={:?}", i, ty));

            assert_eq!(&contract.source, expected_source);
            assert_eq!(
                contract.constructor_args.map(hex::encode),
                expected_contract
                    .constructor_arguments
                    .map(|args| args.trim_start_matches("0x").to_string())
            );
        }
    }
}

fn change_part_for_search(part: &BytecodePart) -> &str {
    part.data.trim_start_matches("0x")
}
