// check-envs.rs
use zetachain_cctx_server::Settings;
use env_collector::{run_env_collector_cli, PrefixFilter};

fn main() {

    //print the current directory
    println!("Current directory: {}", std::env::current_dir().unwrap().display());

    run_env_collector_cli::<Settings>(
        "ZETACHAIN_CCCTX",
        "README.md",
        "config/testnet.toml",
        PrefixFilter::blacklist(&[
            "ZETACHAIN_CCCTX__SERVER",
            "ZETACHAIN_CCCTX__TRACING",
            "ZETACHAIN_CCCTX__JAEGER",
            "ZETACHAIN_CCCTX__METRICS",
        ]),
        None,
    );
}