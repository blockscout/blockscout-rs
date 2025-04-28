use env_collector::{run_env_collector_cli, PrefixFilter};
use tac_operation_lifecycle_server::Settings;

fn main() {
    println!(
        "Current working directory: {}",
        std::env::current_dir().unwrap().display()
    );
    run_env_collector_cli::<Settings>(
        "TAC_OPERATION_LIFECYCLE",
        "../README.md",
        // it's not meant to be read from the file, but it is used to give example
        // values as well as for map var to be generated
        "example_configs/indexer.yaml",
        PrefixFilter::blacklist(&[
            "TAC_OPERATION_LIFECYCLE__SERVER",
            "TAC_OPERATION_LIFECYCLE__TRACING",
            "TAC_OPERATION_LIFECYCLE__JAEGER",
            "TAC_OPERATION_LIFECYCLE__METRICS",
        ]),
        Some("service"),
    );
}
