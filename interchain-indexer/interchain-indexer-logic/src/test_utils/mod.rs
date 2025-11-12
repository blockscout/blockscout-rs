use blockscout_service_launcher::test_database::TestDbGuard;

pub mod mock_db;
pub mod mock_rpc_provider;

pub use mock_rpc_provider::{
    MockRpcProvider, create_pool_with_mock_providers, create_test_node, create_test_node_with_id,
    create_test_pool_config,
};

pub async fn init_db(name: &str) -> TestDbGuard {
    TestDbGuard::new::<migration::Migrator>(name).await
}
