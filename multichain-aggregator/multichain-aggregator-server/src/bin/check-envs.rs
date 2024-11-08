use env_collector::{run_env_collector_cli, PrefixFilter};
use multichain_aggregator_server::Settings;

fn main() {
    run_env_collector_cli::<Settings>(
        "MULTICHAIN_AGGREGATOR",
        "README.md",
        "multichain-aggregator-server/config/example.toml",
        PrefixFilter::blacklist(&[
            "MULTICHAIN_AGGREGATOR__SERVER",
            "MULTICHAIN_AGGREGATOR__JAEGER",
            "MULTICHAIN_AGGREGATOR__METRICS",
            "MULTICHAIN_AGGREGATOR__TRACING",
        ]),
        None,
    );
}
