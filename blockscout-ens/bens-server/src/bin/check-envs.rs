use bens_server::Settings;
use env_collector::{run_env_collector_cli, EnvCollectorSettingsBuilder, PrefixFilter};

fn main() {
    let vars_filter = PrefixFilter::blacklist(&[
        "BENS__SERVER__HTTP__CORS",
        "BENS__SERVER__HTTP__BASE_PATH",
        "BENS__SERVER__GRPC",
        "BENS__DATABASE__CONNECT_OPTIONS",
        "BENS__METRICS",
        "BENS__JAEGER",
        "BENS__SWAGGER_PATH",
    ]);

    let settings = EnvCollectorSettingsBuilder::default()
        .service_name("BENS".to_string())
        .markdown_path("README.md".to_string())
        .config_path("bens-server/config/example.json".to_string())
        .vars_filter(vars_filter)
        .anchor_postfix(Some("envs_main".to_string()))
        .build()
        .expect("invalid settings");
    run_env_collector_cli::<Settings>(settings);
}

// use bens_server::Settings;
// use env_collector::{run_env_collector_cli, PrefixFilter};

// fn main() {
//     run_env_collector_cli::<Settings>(
//         "BENS",
//         "README.md",
//         "bens-server/config/example.json",
//         PrefixFilter::blacklist(&[
//             "BENS__SERVER__HTTP__CORS",
//             "BENS__SERVER__HTTP__BASE_PATH",
//             "BENS__SERVER__GRPC",
//             "BENS__DATABASE__CONNECT_OPTIONS",
//             "BENS__METRICS",
//             "BENS__JAEGER",
//             "BENS__SWAGGER_PATH",
//         ]),
//         Some("envs_main"),
//     );
// }
