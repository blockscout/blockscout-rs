pub mod blobs;
pub mod blocks;
pub mod parser;

use blockscout_service_launcher::test_database::TestDbGuard;

pub async fn init_db(test_name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(test_name).await
}
