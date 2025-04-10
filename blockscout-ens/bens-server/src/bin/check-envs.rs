use bens_server::Settings;
use env_collector::{run_env_collector_cli, PrefixFilter};

fn main() {
    run_env_collector_cli::<Settings>(
        "BENS",
        "README.md",
        "bens-server/config/example.json",
        PrefixFilter::blacklist(&[
            "BENS__SERVER__HTTP__CORS",
            "BENS__SERVER__HTTP__BASE_PATH",
            "BENS__SERVER__GRPC",
            "BENS__DATABASE__CONNECT_OPTIONS",
            "BENS__METRICS",
            "BENS__JAEGER",
        ]),
        Some("envs_main"),
    );
}
