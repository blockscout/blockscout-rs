use env_collector::{EnvCollectorSettingsBuilder, PrefixFilter, run_env_collector_cli};
use interchain_indexer_server::Settings;

fn main() {
    run_env_collector_cli::<Settings>(
        EnvCollectorSettingsBuilder::default()
            .service_name("INTERCHAIN_INDEXER")
            .markdown_path("README.md")
            .config_path("interchain-indexer-server/config/example.toml")
            .vars_filter(PrefixFilter::blacklist(&[
                "INTERCHAIN_INDEXER__SERVER",
                "INTERCHAIN_INDEXER__JAEGER",
            ]))
            .build()
            .expect("failed to build env collector settings"),
    );
}
