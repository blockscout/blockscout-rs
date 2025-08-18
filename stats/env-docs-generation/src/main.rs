use env_collector::{EnvCollectorSettingsBuilder, PrefixFilter, run_env_collector_cli};
use stats_server::{Settings, config_env};

fn main() {
    let mut settings = EnvCollectorSettingsBuilder::default();
    settings
        .service_name("STATS".to_string())
        .markdown_path("README.md".to_string());
    run_env_collector_cli::<Settings>(
        settings
            // it's not meant to be read from the file, but it is used to give example
            // values as well as for map var to be generated
            .config_path("env-docs-generation/example_configs/service.json")
            .vars_filter(PrefixFilter::blacklist(&[
                "STATS__SERVER",
                "STATS__TRACING",
                "STATS__JAEGER",
                "STATS__METRICS",
                "STATS__SWAGGER_PATH",
            ]))
            .anchor_postfix(Some("service".to_string()))
            .build()
            .unwrap(),
    );
    run_env_collector_cli::<Settings>(
        settings
            .config_path("env-docs-generation/example_configs/empty.json")
            .vars_filter(PrefixFilter::whitelist(&["STATS__SERVER"]))
            .anchor_postfix(Some("server".to_string()))
            .build()
            .unwrap(),
    );
    run_env_collector_cli::<Settings>(
        settings
            .config_path("env-docs-generation/example_configs/empty.json")
            .vars_filter(PrefixFilter::whitelist(&[
                "STATS__TRACING",
                "STATS__JAEGER",
            ]))
            .anchor_postfix(Some("tracing".to_string()))
            .build()
            .unwrap(),
    );
    run_env_collector_cli::<Settings>(
        settings
            .config_path("env-docs-generation/example_configs/empty.json")
            .vars_filter(PrefixFilter::whitelist(&["STATS__METRICS"]))
            .anchor_postfix(Some("metrics".to_string()))
            .build()
            .unwrap(),
    );

    run_env_collector_cli::<config_env::charts::Config>(
        settings
            .service_name("STATS_CHARTS".to_string())
            .config_path("env-docs-generation/example_configs/charts.json")
            // setting counter's resolutions will be rejected at
            // the launch anyway because it doesn't make sense
            .vars_filter(PrefixFilter::blacklist(&[
                "STATS_CHARTS__COUNTERS__<COUNTER_NAME>__RESOLUTIONS",
            ]))
            .anchor_postfix(Some("charts".to_string()))
            .build()
            .unwrap(),
    );

    run_env_collector_cli::<config_env::layout::Config>(
        settings
            .service_name("STATS_LAYOUT".to_string())
            .config_path("env-docs-generation/example_configs/layout.json")
            .vars_filter(PrefixFilter::Empty)
            .anchor_postfix(Some("layout".to_string()))
            .build()
            .unwrap(),
    );

    run_env_collector_cli::<config_env::update_groups::Config>(
        settings
            .service_name("STATS_UPDATE_GROUPS".to_string())
            .config_path("env-docs-generation/example_configs/update_groups.json")
            .vars_filter(PrefixFilter::Empty)
            .anchor_postfix(Some("groups".to_string()))
            .build()
            .unwrap(),
    );
}
