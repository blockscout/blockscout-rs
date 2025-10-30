use blockscout_service_launcher::test_database::TestDbGuard;

pub mod mock_db;

pub async fn init_db(name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(name).await
}