use blockscout_service_launcher::test_database::TestDbGuard;

// TODO: When interchain-indexer will be merged into the main branch, rework this approach.
// The interchain indexer DB schema is not in the current branch, so we define a local migrator
// here that creates only the crosschain_messages table for tests. Once the indexer crate is
// available, use its Migrator (e.g. TestDbGuard::new::<interchain_indexer_migration::Migrator>)
// and remove this module.

mod interchain_migrator {
    use sea_orm::Statement;
    use sea_orm_migration::prelude::*;

    #[derive(DeriveMigrationName)]
    pub struct Migration;

    #[async_trait::async_trait]
    impl MigrationTrait for Migration {
        async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    r#"
                    CREATE TABLE crosschain_messages (
                        id BIGSERIAL PRIMARY KEY,
                        init_timestamp TIMESTAMPTZ NOT NULL,
                        src_chain_id BIGINT NOT NULL,
                        dst_chain_id BIGINT NOT NULL,
                        src_tx_hash BYTEA,
                        dst_tx_hash BYTEA
                    )
                    "#
                    .to_string(),
                ))
                .await?;
            Ok(())
        }

        async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    "DROP TABLE IF EXISTS crosschain_messages".to_string(),
                ))
                .await?;
            Ok(())
        }
    }

    pub struct MigratorInterchain;

    #[async_trait::async_trait]
    impl MigratorTrait for MigratorInterchain {
        fn migrations() -> Vec<Box<dyn MigrationTrait>> {
            vec![Box::new(Migration)]
        }
    }
}

pub async fn init_db_all(name: &str) -> (TestDbGuard, TestDbGuard) {
    let db = init_db(name).await;
    let blockscout = init_db_blockscout(name).await;
    (db, blockscout)
}

pub async fn init_db_all_multichain(name: &str) -> (TestDbGuard, TestDbGuard) {
    let db = init_db(name).await;
    let multichain = init_db_multichain(name).await;
    (db, multichain)
}

pub async fn init_db(name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(name).await
}

pub async fn init_db_blockscout(name: &str) -> TestDbGuard {
    TestDbGuard::new::<blockscout_db::migration::Migrator>(&(name.to_owned() + "_blockscout")).await
}

pub async fn init_db_multichain(name: &str) -> TestDbGuard {
    TestDbGuard::new::<multichain_aggregator_migration::Migrator>(
        &(name.to_owned() + "_multichain"),
    )
    .await
}

pub async fn init_db_zetachain_cctx(name: &str) -> TestDbGuard {
    TestDbGuard::new::<zetachain_cctx_migration::Migrator>(&(name.to_owned() + "_zetachain_cctx"))
        .await
}

pub async fn init_db_interchain(name: &str) -> TestDbGuard {
    TestDbGuard::new::<interchain_migrator::MigratorInterchain>(&(name.to_owned() + "_interchain"))
        .await
}

pub async fn init_db_all_interchain(name: &str) -> (TestDbGuard, TestDbGuard) {
    let db = init_db(name).await;
    let interchain = init_db_interchain(name).await;
    (db, interchain)
}
