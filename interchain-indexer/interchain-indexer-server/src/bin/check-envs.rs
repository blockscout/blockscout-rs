use env_collector::{EnvCollectorSettingsBuilder, PrefixFilter, run_env_collector_cli};
use interchain_indexer_server::Settings;

fn main() {
    let mut settings = EnvCollectorSettingsBuilder::default();
    settings
        .service_name("INTERCHAIN_INDEXER".to_string())
        .markdown_path("README.md".to_string());

    run_env_collector_cli::<Settings>(
        settings
            .config_path("interchain-indexer-server/config/example.toml")
            .vars_filter(PrefixFilter::blacklist(&[
                "INTERCHAIN_INDEXER__SERVER",
                "INTERCHAIN_INDEXER__TRACING",
                "INTERCHAIN_INDEXER__JAEGER",
                "INTERCHAIN_INDEXER__METRICS",
                "INTERCHAIN_INDEXER__SWAGGER_PATH",
                "INTERCHAIN_INDEXER__DATABASE__CONNECT_OPTIONS",
                "INTERCHAIN_INDEXER__EXAMPLE_INDEXER",
                "INTERCHAIN_INDEXER__AVALANCHE_INDEXER",
            ]))
            .anchor_postfix(Some("service".to_string()))
            .build()
            .expect("failed to build env collector settings"),
    );

    run_env_collector_cli::<Settings>(
        settings
            .config_path("interchain-indexer-server/config/example.toml")
            .vars_filter(PrefixFilter::whitelist(&[
                "INTERCHAIN_INDEXER__AVALANCHE_INDEXER",
            ]))
            .anchor_postfix(Some("avalanche".to_string()))
            .build()
            .expect("failed to build env collector settings"),
    );

    run_env_collector_cli::<Settings>(
        settings
            .config_path("interchain-indexer-server/config/example.toml")
            .vars_filter(PrefixFilter::whitelist(&["INTERCHAIN_INDEXER__METRICS"]))
            .anchor_postfix(Some("metrics".to_string()))
            .build()
            .expect("failed to build env collector settings"),
    );

    run_env_collector_cli::<Settings>(
        settings
            .config_path("interchain-indexer-server/config/example.toml")
            .vars_filter(PrefixFilter::whitelist(&["INTERCHAIN_INDEXER__SERVER"]))
            .anchor_postfix(Some("server".to_string()))
            .build()
            .expect("failed to build env collector settings"),
    );

    run_env_collector_cli::<Settings>(
        settings
            .config_path("interchain-indexer-server/config/example.toml")
            .vars_filter(PrefixFilter::whitelist(&[
                "INTERCHAIN_INDEXER__TRACING",
                "INTERCHAIN_INDEXER__JAEGER",
            ]))
            .anchor_postfix(Some("tracing".to_string()))
            .build()
            .expect("failed to build env collector settings"),
    );
}
