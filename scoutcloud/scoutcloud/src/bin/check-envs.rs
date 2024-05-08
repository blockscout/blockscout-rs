use blockscout_service_launcher::env_collector::run_env_collector_cli;
use scoutcloud::server::Settings;

fn main() {
    run_env_collector_cli::<Settings>(
        "SCOUTCLOUD",
        "README.md",
        "scoutcloud/config/example.toml",
        &["SCOUTCLOUD__SERVER", "SCOUTCLOUD__JAEGER"],
    );
}
