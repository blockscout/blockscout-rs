use blockscout_service_launcher::test_database::TestDbGuard;

pub async fn init_db_all(name: &str) -> (TestDbGuard, TestDbGuard) {
    let db = init_db(name).await;
    let blockscout =
        TestDbGuard::new::<blockscout_db::migration::Migrator>(&(name.to_owned() + "_blockscout"))
            .await;
    (db, blockscout)
}

pub async fn init_db(name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(name).await
}
