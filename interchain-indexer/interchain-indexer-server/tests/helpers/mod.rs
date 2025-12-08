use alloy::{
    network::Ethereum,
    providers::{DynProvider, Provider, ProviderBuilder},
};
use blockscout_service_launcher::{test_database::TestDbGuard, test_server};
use interchain_indexer_server::Settings;
use reqwest::Url;
use std::path::PathBuf;

/// Create a forked Anvil provider for the given RPC URL and block number.
pub fn forked_provider(rpc_url: &str, block_number: u64) -> DynProvider<Ethereum> {
    ProviderBuilder::new()
        .connect_anvil_with_config(|anvil| anvil.fork_block_number(block_number).fork(rpc_url))
        .erased()
}

pub async fn init_db(db_prefix: &str, test_name: &str) -> TestDbGuard {
    let db_name = format!("{db_prefix}_{test_name}");
    TestDbGuard::new::<migration::Migrator>(db_name.as_str()).await
}
pub async fn init_interchain_indexer_server<F>(db_url: String, settings_setup: F) -> Url
where
    F: Fn(Settings) -> Settings,
{
    let (settings, base) = {
        let mut settings = Settings::default(db_url);
        let (server_settings, base) = test_server::get_test_server_settings();
        settings.server = server_settings;
        settings.metrics.enabled = false;
        settings.tracing.enabled = false;
        settings.jaeger.enabled = false;

        // Resolve config paths relative to workspace root
        // CARGO_MANIFEST_DIR points to interchain-indexer-server/, so we go up one level
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let workspace_root = manifest_dir.parent().unwrap();
        settings.chains_config = workspace_root.join("config/omnibridge/chains.json");
        settings.bridges_config = workspace_root.join("config/omnibridge/bridges.json");

        (settings_setup(settings), base)
    };

    test_server::init_server(|| interchain_indexer_server::run(settings), &base).await;
    base
}
