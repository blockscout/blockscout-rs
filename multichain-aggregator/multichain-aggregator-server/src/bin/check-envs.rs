use env_collector::{EnvCollectorSettingsBuilder, PrefixFilter, run_env_collector_cli};
use multichain_aggregator_server::Settings;

fn main() {
    let vars_filter = PrefixFilter::blacklist(&[
        "MULTICHAIN_AGGREGATOR__SERVER",
        "MULTICHAIN_AGGREGATOR__JAEGER",
        "MULTICHAIN_AGGREGATOR__METRICS",
        "MULTICHAIN_AGGREGATOR__TRACING",
        "MULTICHAIN_AGGREGATOR__DATABASE__CONNECT_OPTIONS",
    ]);

    let settings = EnvCollectorSettingsBuilder::default()
        .service_name("MULTICHAIN_AGGREGATOR".to_string())
        .markdown_path("README.md".to_string())
        .config_path("multichain-aggregator-server/config/example.toml".to_string())
        .vars_filter(vars_filter)
        .anchor_postfix(None)
        .build()
        .expect("invalid settings");

    run_env_collector_cli::<Settings>(settings);
}
