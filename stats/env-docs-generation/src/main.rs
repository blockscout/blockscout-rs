use env_collector::{PrefixFilter, run_env_collector_cli};
use stats_server::{Settings, config_env};

fn main() {
    run_env_collector_cli::<Settings>(
        "STATS",
        "README.md",
        // it's not meant to be read from the file, but it is used to give example
        // values as well as for map var to be generated
        "env-docs-generation/example_configs/service.json",
        PrefixFilter::blacklist(&[
            "STATS__SERVER",
            "STATS__TRACING",
            "STATS__JAEGER",
            "STATS__METRICS",
            "STATS__SWAGGER_PATH",
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
