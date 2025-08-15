use blockscout_service_launcher::test_database::TestDbGuard;

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
