use blockscout_service_launcher::test_database::TestDbGuard;

pub use crate::s3_storage_test_helpers::*;

pub async fn init_db(test_name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(test_name).await
}
