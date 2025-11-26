use env_collector::{EnvCollectorSettingsBuilder, PrefixFilter, run_env_collector_cli};
use user_ops_indexer_server::Settings;

fn main() {
    let mut settings = EnvCollectorSettingsBuilder::default();
    settings
        .service_name("USER_OPS_INDEXER")
        .markdown_path("README.md");
    run_env_collector_cli::<Settings>(
        settings
            .config_path("env-docs-generation/example-configs/service.json")
            .vars_filter(PrefixFilter::blacklist(&[
                "USER_OPS_INDEXER__SERVER",
                "USER_OPS_INDEXER__METRICS",
                "USER_OPS_INDEXER__TRACING",
                "USER_OPS_INDEXER__JAEGER",
                "USER_OPS_INDEXER__SWAGGER_PATH",
            ]))
            .build()
            .unwrap(),
    )
}
