// check-envs.rs
use zetachain_cctx_server::Settings;
use env_collector::{run_env_collector_cli, PrefixFilter};

fn main() {

    run_env_collector_cli::<Settings>(
        "ZETACHAIN_CCCTX",
        "README.md",
        "config/dev/testnet.toml",
        PrefixFilter::blacklist(&[
            "ZETACHAIN_CCCTX__SERVER",
            "ZETACHAIN_CCCTX__TRACING",
            "ZETACHAIN_CCCTX__JAEGER",
            "ZETACHAIN_CCCTX__METRICS",
        ]),
        None,
    );
}