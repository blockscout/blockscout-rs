use {{crate_name}}_server::Settings;
use env_collector::{run_env_collector_cli, EnvCollectorSettingsBuilder, PrefixFilter};

fn main() {
    run_env_collector_cli::<Settings>(
        EnvCollectorSettingsBuilder::default()
            .service_name("{{crate_name | upcase}}")
            .markdown_path("README.md")
            .config_path("{{project-name}}-server/config/example.toml")
            .vars_filter(PrefixFilter::blacklist(&[
                "{{crate_name | upcase}}__SERVER",
                "{{crate_name | upcase}}__JAEGER",
            ]))
            .build()
            .expect("failed to build env collector settings"),
    );
}
