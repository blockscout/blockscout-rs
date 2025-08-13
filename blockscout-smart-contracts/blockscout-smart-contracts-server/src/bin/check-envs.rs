use blockscout_smart_contracts_server::Settings;
use env_collector::{run_env_collector_cli, EnvCollectorSettingsBuilder, PrefixFilter};

fn main() {
    run_env_collector_cli::<Settings>(
        EnvCollectorSettingsBuilder::default()
            .service_name("BLOCKSCOUT_SMART_CONTRACTS")
            .markdown_path("README.md")
            .config_path("blockscout-smart-contracts-server/config/base.toml")
            .vars_filter(PrefixFilter::blacklist(&[
                "BLOCKSCOUT_SMART_CONTRACTS__SERVER",
                "BLOCKSCOUT_SMART_CONTRACTS__JAEGER",
            ]))
            .build()
            .expect("failed to build env collector settings"),
    );
}
