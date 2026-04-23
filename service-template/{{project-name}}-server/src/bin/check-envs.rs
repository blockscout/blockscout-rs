use {{crate_name}}_server::Settings;
use env_collector::{run_env_collector_cli, EnvCollectorSettingsBuilder, PrefixFilter};

fn main() {
    run_env_collector_cli::<Settings>(
        EnvCollectorSettingsBuilder::default()
            .service_name("{{CRATE_NAME}}")
            .markdown_path("README.md")
            .config_path("{{project-name}}-server/config/example.toml")
            .vars_filter(PrefixFilter::blacklist(&[
                "{{CRATE_NAME}}__SERVER",
                "{{CRATE_NAME}}__JAEGER",
                "{{CRATE_NAME}}__METRICS",
                "{{CRATE_NAME}}__TRACING"
            ]))
            .build()
            .expect("failed to build env collector settings"),
    );
}
