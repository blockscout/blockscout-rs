use env_collector::{run_env_collector_cli, PrefixFilter};
use stats_server::{config_env, Settings};

fn main() {
    run_env_collector_cli::<Settings>(
        "STATS",
        "README.md",
        "env-docs-generation/example_configs/empty.json",
        PrefixFilter::blacklist(&[
            "STATS__SERVER",
            "STATS__TRACING",
            "STATS__JAEGER",
            "STATS__METRICS",
        ]),
        Some("service"),
    );
    run_env_collector_cli::<Settings>(
        "STATS",
        "README.md",
        "env-docs-generation/example_configs/empty.json",
        PrefixFilter::whitelist(&["STATS__SERVER"]),
        Some("server"),
    );
    run_env_collector_cli::<Settings>(
        "STATS",
        "README.md",
        "env-docs-generation/example_configs/empty.json",
        PrefixFilter::whitelist(&["STATS__TRACING", "STATS__JAEGER"]),
        Some("tracing"),
    );
    run_env_collector_cli::<Settings>(
        "STATS",
        "README.md",
        "env-docs-generation/example_configs/empty.json",
        PrefixFilter::whitelist(&["STATS__METRICS"]),
        Some("metrics"),
    );

    run_env_collector_cli::<config_env::charts::Config>(
        "STATS_CHARTS",
        "README.md",
        "env-docs-generation/example_configs/charts.json",
        // setting counter's resolutions will be rejected at
        // the launch anyway because it doesn't make sense
        PrefixFilter::blacklist(&["STATS_CHARTS__COUNTERS__<COUNTER_NAME>__RESOLUTIONS"]),
        Some("charts"),
    );

    run_env_collector_cli::<config_env::layout::Config>(
        "STATS_LAYOUT",
        "README.md",
        "env-docs-generation/example_configs/layout.json",
        PrefixFilter::Empty,
        Some("layout"),
    );

    run_env_collector_cli::<config_env::update_groups::Config>(
        "STATS_UPDATE_GROUPS",
        "README.md",
        "env-docs-generation/example_configs/update_groups.json",
        PrefixFilter::Empty,
        Some("groups"),
    );
}
